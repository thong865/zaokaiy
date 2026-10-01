<script setup lang="ts">
import type { Campaign, PromoChannel, PromoTrigger, RewardType } from '../../utils/promo'
import { PROMO_CHANNELS, SIGNUP_METHODS } from '../../utils/promo'

/** Create / edit a campaign. `platform` = admin (sign-up + code), otherwise a shop (first order + code). */
const props = defineProps<{ platform?: boolean; currency: string; initial?: Campaign | null; busy?: boolean }>()
const emit = defineEmits<{ save: [body: Record<string, unknown>]; cancel: [] }>()
const { t } = useI18n()

const triggers = computed<PromoTrigger[]>(() => (props.platform ? ['signup', 'code'] : ['first_order', 'code']))
const rewards = computed<RewardType[]>(() => (props.platform ? ['amount', 'percent', 'free_shipping'] : ['amount', 'percent', 'free_shipping', 'points']))
const toMajor = (c: number) => (c ? String(c / 100) : '')
const toCents = (v: string | number) => Math.round((Number(v) || 0) * 100)
const localInput = (iso: string | null | undefined) => {
  if (!iso) return ''
  const d = new Date(iso)
  return new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16)
}

const c = props.initial
const f = reactive({
  name: c?.name ?? '',
  description: c?.description ?? '',
  trigger: (c?.trigger ?? (props.platform ? 'signup' : 'code')) as PromoTrigger,
  providers: [...(c?.providers ?? (props.platform ? ['facebook', 'whatsapp', 'google'] : []))] as string[],
  code: c?.code ?? '',
  reward_type: (c?.reward_type ?? 'amount') as RewardType,
  amount: c?.reward_type === 'amount' ? toMajor(c.reward_value) : '',
  percent: c?.reward_type === 'percent' ? String(c.reward_value / 100) : '',
  points: c?.reward_type === 'points' ? String(c.reward_value) : '',
  max_discount: toMajor(c?.max_discount_cents ?? 0),
  min_spend: toMajor(c?.min_spend_cents ?? 0),
  currency: c?.currency ?? props.currency,
  coupon_days: c?.coupon_days ?? 30,
  max_grants: c?.max_grants ?? 0,
  channels: [...(c?.channels ?? PROMO_CHANNELS)] as PromoChannel[],
  starts_at: localInput(c?.starts_at),
  ends_at: localInput(c?.ends_at),
  active: c?.active ?? true,
})
// Bonus points only come with first-order campaigns.
watch(() => f.trigger, (tr) => { if (tr !== 'first_order' && f.reward_type === 'points') f.reward_type = 'amount' })
const cur = computed(() => (props.platform ? f.currency : props.currency))

function toggle<T>(list: T[], v: T) {
  const i = list.indexOf(v)
  if (i >= 0) list.splice(i, 1)
  else list.push(v)
}

function submit() {
  const value = f.reward_type === 'amount' ? toCents(f.amount) : f.reward_type === 'percent' ? Math.round((Number(f.percent) || 0) * 100) : f.reward_type === 'points' ? Math.round(Number(f.points) || 0) : 0
  emit('save', {
    name: f.name,
    description: f.description,
    trigger: f.trigger,
    providers: f.trigger === 'signup' ? f.providers : [],
    code: f.trigger === 'code' ? f.code : undefined,
    reward_type: f.reward_type,
    reward_value: value,
    max_discount_cents: f.reward_type === 'percent' ? toCents(f.max_discount) : 0,
    min_spend_cents: toCents(f.min_spend),
    currency: props.platform ? f.currency : undefined,
    coupon_days: Number(f.coupon_days) || 30,
    max_grants: Number(f.max_grants) || 0,
    channels: f.channels,
    starts_at: f.starts_at ? new Date(f.starts_at).toISOString() : undefined,
    ends_at: f.ends_at ? new Date(f.ends_at).toISOString() : null,
    active: f.active,
  })
}
</script>

<template>
  <form class="space-y-4" @submit.prevent="submit">
    <UFormField :label="$t('promo.form.name')" required>
      <UInput v-model="f.name" class="w-full" maxlength="120" :placeholder="platform ? $t('promo.form.namePlatformPh') : $t('promo.form.namePh')" />
    </UFormField>
    <UFormField :label="$t('promo.form.description')" :hint="$t('common.optional')">
      <UTextarea v-model="f.description" class="w-full" :rows="2" :placeholder="$t('promo.form.descriptionPh')" />
    </UFormField>

    <fieldset>
      <legend class="text-sm font-semibold">{{ $t('promo.form.when') }}</legend>
      <div class="mt-2 grid gap-2 sm:grid-cols-2">
        <label v-for="tr in triggers" :key="tr" class="flex cursor-pointer items-start gap-2 rounded-xl border p-3 text-sm" :class="f.trigger === tr ? 'border-brand-500 bg-brand-50/40' : 'border-line'">
          <input v-model="f.trigger" type="radio" :value="tr" class="mt-1">
          <span><span class="font-semibold">{{ $t(`promo.trigger.${tr}`) }}</span><span class="block text-xs text-muted">{{ $t(`promo.trigger.${tr}Hint`) }}</span></span>
        </label>
      </div>
    </fieldset>

    <div v-if="f.trigger === 'signup'">
      <div class="text-sm font-semibold">{{ $t('promo.form.signupWith') }}</div>
      <div class="mt-2 flex flex-wrap gap-2">
        <button v-for="m in SIGNUP_METHODS" :key="m" type="button" class="rounded-full border px-3 py-1 text-sm font-semibold" :class="f.providers.includes(m) ? 'border-brand-500 bg-brand-500 text-white' : 'border-line'" @click="toggle(f.providers, m)">
          {{ $t(`promo.method.${m}`) }}
        </button>
      </div>
      <p class="mt-1 text-xs text-muted">{{ f.providers.length ? $t('promo.form.signupHint') : $t('promo.form.signupAny') }}</p>
    </div>
    <UFormField v-if="f.trigger === 'code'" :label="$t('promo.form.code')" :hint="$t('promo.form.codeHint')" required>
      <UInput v-model="f.code" class="w-full font-mono uppercase" maxlength="20" placeholder="LIVE10" @update:model-value="(v: string) => (f.code = String(v).toUpperCase().replace(/[^A-Z0-9]/g, ''))" />
    </UFormField>

    <fieldset>
      <legend class="text-sm font-semibold">{{ $t('promo.form.reward') }}</legend>
      <div class="mt-2 flex flex-wrap gap-2">
        <button v-for="r in rewards" :key="r" type="button" :disabled="r === 'points' && f.trigger !== 'first_order'" class="rounded-full border px-3 py-1 text-sm font-semibold disabled:opacity-40" :class="f.reward_type === r ? 'border-brand-500 bg-brand-500 text-white' : 'border-line'" @click="f.reward_type = r">
          {{ $t(`promo.rewardType.${r}`) }}
        </button>
      </div>
      <div class="mt-3 grid gap-3 sm:grid-cols-2">
        <UFormField v-if="platform" :label="$t('promo.form.currency')">
          <select v-model="f.currency" class="input"><option>LAK</option><option>THB</option><option>USD</option></select>
        </UFormField>
        <UFormField v-if="f.reward_type === 'amount'" :label="$t('promo.form.amount', { cur })" required>
          <UInput v-model="f.amount" type="number" min="0" step="0.01" class="w-full" />
        </UFormField>
        <template v-if="f.reward_type === 'percent'">
          <UFormField :label="$t('promo.form.percent')" required>
            <UInput v-model="f.percent" type="number" min="0.01" max="100" step="0.01" class="w-full" />
          </UFormField>
          <UFormField :label="$t('promo.form.maxDiscount', { cur })" :hint="$t('promo.form.zeroNoLimit')">
            <UInput v-model="f.max_discount" type="number" min="0" step="0.01" class="w-full" />
          </UFormField>
        </template>
        <UFormField v-if="f.reward_type === 'points'" :label="$t('promo.form.points')" required>
          <UInput v-model="f.points" type="number" min="1" step="1" class="w-full" />
        </UFormField>
        <UFormField v-if="f.reward_type !== 'points'" :label="$t('promo.form.minSpend', { cur })" :hint="$t('promo.form.zeroNoLimit')">
          <UInput v-model="f.min_spend" type="number" min="0" step="0.01" class="w-full" />
        </UFormField>
        <UFormField v-if="f.reward_type !== 'points' && f.trigger !== 'code'" :label="$t('promo.form.couponDays')">
          <UInput v-model.number="f.coupon_days" type="number" min="1" max="3650" class="w-full" />
        </UFormField>
      </div>
      <p v-if="platform" class="mt-2 rounded-xl bg-sun-400/15 p-2.5 text-xs text-amber-900">{{ $t('promo.form.platformPays') }}</p>
    </fieldset>

    <div>
      <div class="text-sm font-semibold">{{ $t('promo.form.channels') }}</div>
      <div class="mt-2 flex flex-wrap gap-2">
        <button v-for="ch in PROMO_CHANNELS" :key="ch" type="button" class="rounded-full border px-3 py-1 text-sm font-semibold" :class="f.channels.includes(ch) ? 'border-ink bg-ink text-paper' : 'border-line'" @click="toggle(f.channels, ch)">
          {{ $t(`promo.channel.${ch}`) }}
        </button>
      </div>
    </div>

    <div class="grid gap-3 sm:grid-cols-3">
      <UFormField :label="$t('promo.form.starts')"><UInput v-model="f.starts_at" type="datetime-local" class="w-full" /></UFormField>
      <UFormField :label="$t('promo.form.ends')" :hint="$t('common.optional')"><UInput v-model="f.ends_at" type="datetime-local" class="w-full" /></UFormField>
      <UFormField :label="$t('promo.form.budget')" :hint="$t('promo.form.zeroNoLimit')"><UInput v-model.number="f.max_grants" type="number" min="0" class="w-full" /></UFormField>
    </div>
    <label class="flex items-center gap-2 text-sm font-semibold"><USwitch v-model="f.active" size="sm" /> {{ $t('promo.form.active') }}</label>

    <div class="flex justify-end gap-2 pt-1">
      <UButton color="neutral" variant="soft" @click="emit('cancel')">{{ $t('common.cancel') }}</UButton>
      <UButton type="submit" :loading="busy">{{ $t('common.save') }}</UButton>
    </div>
  </form>
</template>
