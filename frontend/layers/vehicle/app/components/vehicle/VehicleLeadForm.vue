<script setup lang="ts">
import type { ShopContact, VehicleSpec } from '~/utils/types'

const props = defineProps<{ productId: string; productName: string; price: number; currency: string; spec: VehicleSpec; contact: ShopContact | null; initialKind?: string }>()
const api = useApi()
const { t } = useI18n()
const { user } = useAuth()

const kinds = computed(() =>
  LEAD_KINDS.filter((k) => (k !== 'reserve' || props.spec.sale_status === 'available') && (k !== 'finance' || props.spec.finance_available)),
)
const kind = ref<string>(props.initialKind && LEAD_KINDS.includes(props.initialKind as never) ? props.initialKind : 'enquiry')
watch(() => props.initialKind, (k) => { if (k) kind.value = k })
const f = reactive({ name: user.value?.display_name ?? '', phone: user.value?.phone ?? '', message: '', when: '', offer: '' })
const busy = ref(false)
const error = ref('')
const done = ref<{ kind: string; deposit_cents: number; currency: string; payment_instructions: string; contact: ShopContact; duplicate: boolean } | null>(null)

// datetime-local wants local "YYYY-MM-DDTHH:mm"; default: tomorrow 10:00
function localInput(d: Date) {
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`
}
const minWhen = ref('')
onMounted(() => {
  const d = new Date()
  minWhen.value = localInput(d)
  d.setDate(d.getDate() + 1)
  d.setHours(10, 0, 0, 0)
  if (!f.when) f.when = localInput(d)
})

async function send() {
  busy.value = true
  error.value = ''
  try {
    const body: Record<string, unknown> = { kind: kind.value, name: f.name, phone: f.phone, message: f.message }
    if (kind.value === 'test_drive' && f.when) body.preferred_at = new Date(f.when).toISOString()
    if (kind.value === 'offer') body.offer_cents = Math.round(Number(f.offer) * 100) || null
    done.value = await api(`/catalog/products/${props.productId}/leads`, { method: 'POST', body })
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
const hint = computed(() => {
  if (kind.value === 'reserve')
    return props.spec.deposit_cents > 0 ? t('vehicle.lead.reserveHint', { amount: money(props.spec.deposit_cents, props.currency) }) : t('vehicle.lead.reserveNoDeposit')
  if (kind.value === 'finance') return t('vehicle.lead.financeHint')
  if (kind.value === 'offer') return t('vehicle.lead.offerHint', { price: money(props.price, props.currency) })
  return ''
})
const waText = computed(() => t('vehicle.lead.waFollowUp', { name: props.productName }))
</script>

<template>
  <section class="card p-5" aria-labelledby="lead-title">
    <template v-if="!done">
      <h2 id="lead-title" class="font-bold">{{ $t('vehicle.lead.title') }}</h2>
      <div class="mt-3 flex flex-wrap gap-1.5" role="tablist">
        <button
          v-for="k in kinds"
          :key="k"
          type="button"
          role="tab"
          :aria-selected="kind === k"
          class="chip border px-3 py-1.5"
          :class="kind === k ? 'border-brand-500 bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]' : 'border-transparent bg-surface shadow-card hover:text-brand-500'"
          @click="kind = k"
        >
          {{ $t(`vehicle.lead.kinds.${k}`) }}
        </button>
      </div>
      <form class="mt-4 space-y-3" @submit.prevent="send">
        <p v-if="hint" class="rounded-xl bg-paper p-3 text-sm text-ink/80">{{ hint }}</p>
        <div class="grid gap-3 sm:grid-cols-2">
          <div><label for="lead-name" class="label">{{ $t('vehicle.lead.name') }}</label><UInput id="lead-name" v-model="f.name" required maxlength="80" autocomplete="name" /></div>
          <div><label for="lead-phone" class="label">{{ $t('vehicle.lead.phone') }}</label><UInput id="lead-phone" v-model="f.phone" required type="tel" :placeholder="$t('vehicle.lead.phonePh')" autocomplete="tel" /></div>
        </div>
        <div v-if="kind === 'test_drive'"><label for="lead-when" class="label">{{ $t('vehicle.lead.when') }}</label><UInput id="lead-when" v-model="f.when" required type="datetime-local" :min="minWhen" /></div>
        <div v-if="kind === 'offer'">
          <label for="lead-offer" class="label">{{ $t('vehicle.lead.offer', { currency }) }}</label>
          <UInput id="lead-offer" v-model.number="f.offer" required type="number" min="1" />
        </div>
        <div><label for="lead-msg" class="label">{{ $t('vehicle.lead.message') }} <span class="normal-case tracking-normal text-muted">({{ $t('common.optional') }})</span></label><UTextarea id="lead-msg" v-model="f.message" :rows="3" maxlength="2000" :placeholder="$t(`vehicle.lead.messagePh.${kind}`)" /></div>
        <p v-if="error" class="text-sm text-brand-700" role="alert">{{ error }}</p>
        <UButton type="submit" class="w-full" :disabled="busy">{{ busy ? $t('vehicle.lead.sending') : $t(`vehicle.lead.send.${kind}`) }}</UButton>
        <p class="text-center text-xs text-muted">{{ $t('vehicle.lead.privacy') }}</p>
      </form>
    </template>
    <div v-else class="space-y-3" role="status">
      <div class="grid size-11 place-items-center rounded-2xl bg-mint-500 text-xl text-white">✓</div>
      <h2 class="text-lg font-bold">{{ $t('vehicle.lead.sentTitle') }}</h2>
      <p class="text-sm text-ink/80">{{ $t('vehicle.lead.sentText', { shop: done.contact.name, phone: f.phone }) }}</p>
      <div v-if="done.kind === 'reserve' && done.deposit_cents > 0" class="rounded-xl bg-sun-400/20 p-3 text-sm">
        <div class="font-bold">{{ $t('vehicle.lead.payTitle', { amount: money(done.deposit_cents, done.currency) }) }}</div>
        <p v-if="done.payment_instructions" class="mt-1 whitespace-pre-line">{{ done.payment_instructions }}</p>
        <p class="mt-1 text-xs text-muted">{{ $t('vehicle.lead.payNote') }}</p>
      </div>
      <div class="flex flex-wrap gap-2">
        <UButton color="neutral" size="sm" v-if="waLink(done.contact.phone, waText)" :to="waLink(done.contact.phone, waText)" target="_blank" rel="noopener">{{ $t('vehicle.detail.whatsapp') }}</UButton>
        <UButton color="neutral" variant="soft" size="sm" type="submit" @click="done = null">{{ $t('vehicle.lead.again') }}</UButton>
      </div>
    </div>
  </section>
</template>
