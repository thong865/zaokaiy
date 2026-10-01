<script setup lang="ts">
import type { FeePayer, ShippingOption } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { t, locale } = useI18n()
const { shopId, shop } = useShop()
const cur = computed(() => shop.value?.currency ?? 'LAK')

interface Form {
  enabled: boolean
  fee_payer: FeePayer
  fee: number | string
  free_over: number | string
  home_on: boolean
  home_fee: number | string
  cod_enabled: boolean
  cod_fee: number | string
  eta: string
  note: string
}
const toMajor = (c: number | null | undefined) => (c == null ? '' : c / 100)
const toCents = (v: number | string) => Math.max(0, Math.round(Number(v || 0) * 100))
const fromOption = (o: ShippingOption): Form => ({
  enabled: o.enabled,
  fee_payer: o.fee_payer,
  fee: toMajor(o.fee_cents),
  free_over: toMajor(o.free_over_cents),
  home_on: o.home_fee_cents != null,
  home_fee: toMajor(o.home_fee_cents),
  cod_enabled: o.cod_enabled && o.supports_cod,
  cod_fee: toMajor(o.cod_fee_cents),
  eta: o.eta,
  note: o.note,
})

const { data: options } = await useAsyncData('shop-shipping', () => api<ShippingOption[]>(`/shops/${shopId.value}/shipping`), {
  watch: [shopId],
  default: () => [] as ShippingOption[],
})
const forms = reactive<Record<string, Form>>({})
const msg = reactive<Record<string, string>>({})
const err = reactive<Record<string, string>>({})
const saving = reactive<Record<string, boolean>>({})
watch(options, (list) => {
  for (const o of list) forms[o.carrier_code] = fromOption(o)
}, { immediate: true })

const nameOf = (o: ShippingOption) => (locale.value === 'lo' && o.name_lo ? o.name_lo : o.name)
const anyEnabled = computed(() => options.value.some((o) => o.enabled))
const payers: FeePayer[] = ['buyer', 'destination', 'seller']

const timer: Record<string, ReturnType<typeof setTimeout>> = {}
async function save(o: ShippingOption) {
  const code = o.carrier_code
  const f = forms[code]
  if (!f) return
  msg[code] = err[code] = ''
  saving[code] = true
  try {
    const list = await api<ShippingOption[]>(`/shops/${shopId.value}/shipping/${code}`, {
      method: 'PUT',
      body: {
        enabled: f.enabled,
        fee_payer: f.fee_payer,
        fee_cents: toCents(f.fee),
        home_fee_cents: f.home_on ? toCents(f.home_fee) : null,
        free_over_cents: f.free_over === '' || f.free_over == null ? null : toCents(f.free_over),
        cod_enabled: o.supports_cod && f.cod_enabled,
        cod_fee_cents: toCents(f.cod_fee),
        eta: f.eta.trim(),
        note: f.note.trim(),
      },
    })
    // Keep unsaved edits on the other cards: only refresh this courier's form.
    const fresh = list.find((x) => x.carrier_code === code)
    const i = options.value.findIndex((x) => x.carrier_code === code)
    if (fresh && i >= 0) {
      options.value.splice(i, 1, fresh)
      forms[code] = fromOption(fresh)
    }
    msg[code] = t('common.saved')
    clearTimeout(timer[code])
    timer[code] = setTimeout(() => (msg[code] = ''), 2500)
  } catch (e) {
    err[code] = apiError(e)
  } finally {
    saving[code] = false
  }
}
</script>

<template>
  <div class="max-w-3xl">
    <h1 class="page-title">{{ $t('ship.settings.title') }}</h1>
    <p class="mt-1 text-sm text-muted">{{ $t('ship.settings.intro') }}</p>
    <ol class="mt-4 grid gap-2 text-sm sm:grid-cols-3">
      <li v-for="(k, i) in ['step1', 'step2', 'step3']" :key="k" class="card flex gap-3 p-4">
        <span class="grid size-6 shrink-0 place-items-center rounded-full bg-brand-500 text-xs font-bold text-white">{{ i + 1 }}</span>
        <span>{{ $t(`ship.settings.${k}`) }}</span>
      </li>
    </ol>
    <NuxtLink to="/dashboard/cod" class="mt-2 inline-block text-sm font-semibold text-brand-700 hover:underline">{{ $t('ship.settings.codPage') }} →</NuxtLink>

    <p v-if="options.length && !anyEnabled" class="mt-5 rounded-xl bg-sun-400/25 p-3 text-sm text-amber-800">{{ $t('ship.settings.noneEnabled') }}</p>
    <EmptyState v-if="!options.length" class="mt-5" :title="$t('ship.settings.title')" :text="$t('ship.settings.empty')" />

    <div class="mt-5 space-y-4">
      <form v-for="o in options" :key="o.carrier_code" class="card space-y-4 p-5 sm:p-6" :class="!forms[o.carrier_code]?.enabled && 'opacity-90'" @submit.prevent="save(o)">
        <template v-if="forms[o.carrier_code]">
          <div class="flex flex-wrap items-center gap-3">
            <div>
              <div class="text-lg font-bold">{{ nameOf(o) }}</div>
              <div class="flex flex-wrap gap-x-3 text-xs text-muted">
                <span v-if="locale === 'lo' && o.name_lo && o.name_lo !== o.name">{{ o.name }}</span>
                <a v-if="o.website" :href="o.website" target="_blank" rel="noopener" class="hover:text-ink hover:underline">{{ $t('ship.settings.website') }}</a>
                <span v-if="!o.configured" class="chip bg-line text-muted">{{ $t('ship.settings.notConfigured') }}</span>
              </div>
            </div>
            <label class="ml-auto flex cursor-pointer items-center gap-2 text-sm font-semibold">
              {{ $t('ship.settings.enabled') }}
              <span class="relative inline-flex items-center">
                <input v-model="forms[o.carrier_code]!.enabled" type="checkbox" class="peer sr-only">
                <span class="h-6 w-11 rounded-full bg-line transition peer-checked:bg-mint-500" />
                <span class="absolute left-0.5 top-0.5 size-5 rounded-full bg-surface shadow transition peer-checked:translate-x-5" />
              </span>
            </label>
          </div>

          <fieldset :disabled="!forms[o.carrier_code]!.enabled" class="space-y-4 disabled:opacity-60">
            <div class="grid gap-3 sm:grid-cols-2">
              <div>
                <label class="label">{{ $t('ship.settings.fee', { cur }) }}</label>
                <UInput v-model.number="forms[o.carrier_code]!.fee" type="number" min="0" step="any" />
                <p class="mt-1 text-xs text-muted">{{ $t('ship.settings.feeHint') }}</p>
              </div>
              <div>
                <label class="label">{{ $t('ship.settings.freeOver', { cur }) }}</label>
                <UInput v-model.number="forms[o.carrier_code]!.free_over" type="number" min="0" step="any" />
                <p class="mt-1 text-xs text-muted">{{ $t('ship.settings.freeOverHint') }}</p>
              </div>
            </div>

            <div>
              <div class="label">{{ $t('ship.settings.whoPays') }}</div>
              <div class="grid gap-2 sm:grid-cols-3">
                <label v-for="p in payers" :key="p" class="flex cursor-pointer items-start gap-2 rounded-xl border p-3 text-sm" :class="forms[o.carrier_code]!.fee_payer === p ? 'border-ink bg-paper' : 'border-line'">
                  <input v-model="forms[o.carrier_code]!.fee_payer" type="radio" :value="p" class="mt-0.5 size-4 accent-brand-500">
                  <span>{{ $t(`ship.settings.payer.${p}`) }}</span>
                </label>
              </div>
            </div>

            <div class="grid gap-3 sm:grid-cols-2">
              <div class="rounded-xl border border-line p-3">
                <label class="flex items-center gap-2 text-sm font-semibold">
                  <input v-model="forms[o.carrier_code]!.home_on" type="checkbox" class="size-4 accent-brand-500">
                  {{ $t('ship.settings.home') }}
                </label>
                <div v-if="forms[o.carrier_code]!.home_on" class="mt-2">
                  <label class="label">{{ $t('ship.settings.homeFee', { cur }) }}</label>
                  <UInput v-model.number="forms[o.carrier_code]!.home_fee" type="number" min="0" step="any" />
                </div>
                <p v-else class="mt-1 text-xs text-muted">{{ $t('ship.settings.homeOff') }}</p>
              </div>
              <div class="rounded-xl border border-line p-3">
                <label class="flex items-center gap-2 text-sm font-semibold" :class="!o.supports_cod && 'text-muted'">
                  <input v-model="forms[o.carrier_code]!.cod_enabled" type="checkbox" class="size-4 accent-brand-500" :disabled="!o.supports_cod">
                  {{ $t('ship.settings.cod') }}
                </label>
                <p v-if="!o.supports_cod" class="mt-1 text-xs text-muted">{{ $t('ship.settings.codUnsupported') }}</p>
                <div v-else-if="forms[o.carrier_code]!.cod_enabled" class="mt-2">
                  <label class="label">{{ $t('ship.settings.codFee', { cur }) }}</label>
                  <UInput v-model.number="forms[o.carrier_code]!.cod_fee" type="number" min="0" step="any" />
                </div>
              </div>
            </div>

            <div class="grid gap-3 sm:grid-cols-[180px_1fr]">
              <div>
                <label class="label">{{ $t('ship.settings.eta') }}</label>
                <UInput v-model="forms[o.carrier_code]!.eta" maxlength="60" :placeholder="$t('ship.settings.etaPlaceholder')" />
              </div>
              <div>
                <label class="label">{{ $t('ship.settings.note') }}</label>
                <UInput v-model="forms[o.carrier_code]!.note" maxlength="300" :placeholder="$t('ship.settings.notePlaceholder')" />
              </div>
            </div>
          </fieldset>

          <div class="flex flex-wrap items-center gap-3 border-t border-line pt-4">
            <UButton size="sm" type="submit" :disabled="saving[o.carrier_code]">{{ saving[o.carrier_code] ? $t('common.saving') : $t('ship.settings.save') }}</UButton>
            <span v-if="msg[o.carrier_code]" class="text-sm text-mint-500">{{ msg[o.carrier_code] }}</span>
            <span v-if="err[o.carrier_code]" class="text-sm text-brand-700">{{ err[o.carrier_code] }}</span>
          </div>
        </template>
      </form>
    </div>
  </div>
</template>
