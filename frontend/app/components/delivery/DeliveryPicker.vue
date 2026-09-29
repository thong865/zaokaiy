<script setup lang="ts">
import type { DeliveryType, ShippingOption } from '~/utils/types'
import type { DeliveryChoice, DeliveryQuote } from '~/utils/delivery'

/** Courier picker: one radio card per courier, branch/home switch and pay-now vs cash-on-delivery. */
const props = defineProps<{ options: ShippingOption[]; currency: string; subtotalCents: number; name?: string }>()
const model = defineModel<DeliveryChoice | null>({ default: null })
const emit = defineEmits<{ quote: [q: DeliveryQuote | null] }>()
const { t, locale } = useI18n()
const uid = useId()
const group = computed(() => props.name || `delivery-${uid}`)

// Keep the choice valid: auto-select the first courier, drop home/COD the courier doesn't offer.
watch(
  () => [props.options, model.value] as const,
  ([opts, m]) => {
    const o = m && opts.find((x) => x.carrier_code === m.carrier_code)
    if (!o) {
      const d = defaultChoice(opts)
      if (d || m) model.value = d
      return
    }
    const type: DeliveryType = m.delivery_type === 'home' && o.home_fee_cents == null ? 'branch' : m.delivery_type
    const cod = m.cod && codAvailable(o)
    if (type !== m.delivery_type || cod !== m.cod) model.value = { ...m, delivery_type: type, cod }
  },
  { immediate: true, deep: true },
)

const current = computed(() => quoteChoice(props.options, model.value, props.subtotalCents))
watch(current, (q) => emit('quote', q), { immediate: true })

function feeLabel(o: ShippingOption, type: DeliveryType) {
  const q = quote(o, type, false, props.subtotalCents)
  if (q.fee_payer === 'seller') return t('delivery.free')
  if (q.fee_payer === 'destination') return q.fee_cents ? t('delivery.payAtPickupFee', { fee: money(q.fee_cents, props.currency) }) : t('delivery.payAtPickup')
  return money(q.fee_cents, props.currency)
}

const cards = computed(() =>
  props.options.map((o) => {
    const selected = model.value?.carrier_code === o.carrier_code
    const type: DeliveryType = selected ? model.value!.delivery_type : 'branch'
    const q = quote(o, type, false, props.subtotalCents)
    return {
      o,
      selected,
      name: optionName(o, locale.value),
      price: feeLabel(o, type),
      free: q.fee_payer === 'seller',
      freeHint: o.free_over_cents != null && props.subtotalCents < o.free_over_cents ? t('delivery.freeOver', { amount: money(o.free_over_cents, props.currency) }) : '',
      branchLabel: feeLabel(o, 'branch'),
      homeLabel: o.home_fee_cents != null ? feeLabel(o, 'home') : '',
      cod: codAvailable(o),
      codFee: o.cod_fee_cents ? t('delivery.codFeeAmount', { fee: money(o.cod_fee_cents, props.currency) }) : t('delivery.noCodFee'),
    }
  }),
)

function pick(code: string) {
  if (model.value?.carrier_code === code) return
  model.value = { carrier_code: code, delivery_type: 'branch', cod: false }
}
function set(patch: Partial<DeliveryChoice>) {
  if (model.value) model.value = { ...model.value, ...patch }
}
</script>

<template>
  <div class="space-y-2" role="radiogroup" :aria-label="$t('delivery.chooseCourier')">
    <div
      v-for="c in cards"
      :key="c.o.carrier_code"
      class="rounded-2xl border p-3 transition"
      :class="c.selected ? 'border-ink bg-paper' : 'border-line bg-surface hover:border-ink/40'"
    >
      <label class="flex cursor-pointer items-start gap-3">
        <input
          type="radio"
          class="mt-1 accent-brand-500"
          :name="group"
          :value="c.o.carrier_code"
          :checked="c.selected"
          @change="pick(c.o.carrier_code)"
        >
        <span class="min-w-0 flex-1">
          <span class="flex items-start justify-between gap-2">
            <span class="font-semibold">{{ c.name }}</span>
            <span class="shrink-0 text-right text-sm font-bold tabular-nums" :class="c.free ? 'text-mint-500' : ''">{{ c.price }}</span>
          </span>
          <span v-if="c.o.eta" class="block text-xs text-muted">{{ $t('delivery.eta', { eta: c.o.eta }) }}</span>
          <span v-if="c.freeHint" class="block text-xs text-muted">{{ c.freeHint }}</span>
          <span v-if="c.o.note" class="block text-xs text-muted">{{ c.o.note }}</span>
          <span v-if="c.cod && !c.selected" class="chip mt-1 bg-sun-400/20 text-amber-800">{{ $t('delivery.codAvailableShort') }}</span>
        </span>
      </label>

      <div v-if="c.selected && model" class="mt-3 space-y-2 pl-7">
        <div v-if="c.homeLabel" class="grid grid-cols-2 gap-2">
          <button
            type="button"
            class="rounded-xl border px-3 py-2 text-left text-xs"
            :class="model.delivery_type === 'branch' ? 'border-ink bg-surface font-semibold' : 'border-line bg-surface/60'"
            :aria-pressed="model.delivery_type === 'branch'"
            @click="set({ delivery_type: 'branch' })"
          >
            <span class="block">{{ $t('delivery.branch') }}</span>
            <span class="block text-muted">{{ c.branchLabel }}</span>
          </button>
          <button
            type="button"
            class="rounded-xl border px-3 py-2 text-left text-xs"
            :class="model.delivery_type === 'home' ? 'border-ink bg-surface font-semibold' : 'border-line bg-surface/60'"
            :aria-pressed="model.delivery_type === 'home'"
            @click="set({ delivery_type: 'home' })"
          >
            <span class="block">{{ $t('delivery.home') }}</span>
            <span class="block text-muted">{{ c.homeLabel }}</span>
          </button>
        </div>
        <div v-else class="text-xs text-muted">{{ $t('delivery.branchOnly') }}</div>

        <div v-if="c.cod" class="grid grid-cols-2 gap-2">
          <button
            type="button"
            class="rounded-xl border px-3 py-2 text-left text-xs"
            :class="!model.cod ? 'border-ink bg-surface font-semibold' : 'border-line bg-surface/60'"
            :aria-pressed="!model.cod"
            @click="set({ cod: false })"
          >
            <span class="block">{{ $t('delivery.payNow') }}</span>
            <span class="block text-muted">{{ $t('delivery.payNowHint') }}</span>
          </button>
          <button
            type="button"
            class="rounded-xl border px-3 py-2 text-left text-xs"
            :class="model.cod ? 'border-ink bg-surface font-semibold' : 'border-line bg-surface/60'"
            :aria-pressed="model.cod"
            @click="set({ cod: true })"
          >
            <span class="block">{{ $t('delivery.cod') }}</span>
            <span class="block text-muted">{{ c.codFee }}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
