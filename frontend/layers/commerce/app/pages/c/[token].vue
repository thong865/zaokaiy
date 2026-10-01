<script setup lang="ts">
import type { SocialOrderDoc } from '~/utils/types'
import type { DeliveryChoice } from '#commerce/utils/delivery'

/** Customer checkout for a comment/chat order — opened from the link in the auto-reply. No account needed. */
const route = useRoute()
const api = useApi()
const token = String(route.params.token)
const { data: doc, error: loadError } = await useAsyncData(`co:${token}`, () => api<SocialOrderDoc>(`/public/social-orders/${token}`))
if (loadError.value || !doc.value) throw createError({ statusCode: 404, statusMessage: tr('checkout.notFound'), fatal: true })
const { t } = useI18n()
useHead({ title: () => t('checkout.title', { number: doc.value?.order.number ?? '', shop: doc.value?.shop.name ?? '' }), meta: [{ name: 'robots', content: 'noindex' }] })

const o = computed(() => doc.value!.order)
const cur = computed(() => o.value.currency)
// First visit: start from the name on their Facebook/WhatsApp/TikTok profile.
const f = reactive({ name: o.value.ship_name || doc.value!.customer.name, phone: o.value.ship_phone, address: o.value.ship_address, note: o.value.customer_note })
const pay = reactive({ method: o.value.payment_method && o.value.payment_method !== 'cod' ? o.value.payment_method : 'transfer', reference: o.value.payment_ref })
const { locale } = useI18n()
const { trackingLink } = useCarriers()
// The confirm/payment responses don't repeat the shop's couriers — keep the list from the first load.
const shippingOptions = ref(doc.value?.shipping_options ?? [])
const hasOptions = computed(() => shippingOptions.value.length > 0)
const choice = ref<DeliveryChoice | null>(
  o.value.carrier_code && shippingOptions.value.some((x) => x.carrier_code === o.value.carrier_code)
    ? { carrier_code: o.value.carrier_code, delivery_type: o.value.delivery_type, cod: o.value.payment_method === 'cod' }
    : defaultChoice(shippingOptions.value),
)
const isCod = computed(() => o.value.payment_method === 'cod')
/** While editing, preview the fee of the courier being picked; otherwise show what the order says. */
const preview = computed(() => (editing.value && hasOptions.value ? quoteChoice(shippingOptions.value, choice.value, o.value.subtotal_cents) : null))
const shipLine = computed(() => {
  const q = preview.value
  if (q) {
    if (q.fee_payer === 'seller') return t('delivery.free')
    if (q.fee_payer === 'destination') return t('delivery.payAtPickup')
    return money(q.charged_fee_cents, cur.value)
  }
  if (o.value.carrier_code && o.value.fee_payer === 'destination') return t('delivery.payAtPickup')
  return o.value.shipping_cents ? money(o.value.shipping_cents, cur.value) : t('common.free')
})
const codFeeLine = computed(() => (preview.value ? preview.value.cod_fee_cents : o.value.cod_fee_cents))
// Plug-in module `promo` (layers/promo): coupon / points chosen in the `checkout.promo` slot.
const promoHook = useState<{ body: Record<string, unknown> | null; discount: Record<string, number> }>(`zk-promo:order:${token}`, () => ({ body: null, discount: {} }))
const discountLine = computed(() => (editing.value ? promoHook.value.discount[''] ?? 0 : o.value.discount_cents ?? 0))
const totalLine = computed(() =>
  editing.value ? (preview.value ? o.value.subtotal_cents + preview.value.charged_fee_cents + preview.value.cod_fee_cents : o.value.total_cents + (o.value.discount_cents ?? 0)) - discountLine.value : o.value.total_cents,
)
const courierName = computed(() => {
  const opt = shippingOptions.value.find((x) => x.carrier_code === o.value.carrier_code)
  return opt ? optionName(opt, locale.value) : (o.value.carrier_code ?? '')
})
const deliveryTypeText = computed(() => (o.value.carrier_code ? t(`delivery.${o.value.delivery_type}`) : ''))
const trackUrl = computed(() => (o.value.tracking_no ? trackingLink(o.value.carrier_code, o.value.tracking_no) : null))
function setDoc(d: SocialOrderDoc) {
  doc.value = { ...d, shipping_options: d.shipping_options ?? shippingOptions.value }
}
const busy = ref(false)
const error = ref('')
const editing = ref(o.value.status === 'open')
async function confirm() {
  busy.value = true
  error.value = ''
  try {
    const c = hasOptions.value ? choice.value : null
    if (hasOptions.value && !c) throw new Error(t('delivery.chooseCourier'))
    const body = { ...(c ? { ...f, carrier_code: c.carrier_code, delivery_type: c.delivery_type, cod: c.cod } : { ...f }), ...(promoHook.value.body ? { promo: promoHook.value.body } : {}) }
    setDoc(await api<SocialOrderDoc>(`/public/social-orders/${token}/confirm`, { method: 'POST', body }))
    editing.value = false
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
async function sendPayment() {
  busy.value = true
  error.value = ''
  try {
    setDoc(await api<SocialOrderDoc>(`/public/social-orders/${token}/payment`, { method: 'POST', body: pay }))
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
const statusKeys = ['open', 'confirmed', 'paid', 'shipped', 'completed', 'cancelled', 'expired']
const statusText = computed(() =>
  o.value.status === 'confirmed' && isCod.value ? t('delivery.codConfirmedLong') : statusKeys.includes(o.value.status) ? t(`checkout.status.${o.value.status}`) : '',
)
/** Order tracking steps (COD orders skip "paid": they are paid to the courier). */
const timeline = computed(() => {
  const x = o.value
  const rank: Record<string, number> = { open: 0, confirmed: 1, paid: 2, shipped: 3, completed: 4 }
  const r = rank[x.status] ?? -1
  const steps = [
    { key: 'ordered', at: x.created_at, done: true },
    { key: 'confirmed', at: x.confirmed_at, done: r >= 1 },
    ...(isCod.value ? [] : [{ key: 'paid', at: x.paid_at, done: r >= 2 }]),
    { key: 'shipped', at: x.shipped_at, done: r >= 3 },
    { key: 'completed', at: x.status === 'completed' ? x.updated_at : null, done: r >= 4 },
  ]
  return steps
})
// Keep the tracking view fresh when the customer comes back to the tab.
const refreshDoc = async () => {
  if (editing.value || busy.value) return
  try { setDoc(await api<SocialOrderDoc>(`/public/social-orders/${token}`)) } catch { /* keep what we have */ }
}
onMounted(() => window.addEventListener('focus', refreshDoc))
onBeforeUnmount(() => window.removeEventListener('focus', refreshDoc))
const remaining = computed(() => {
  const ms = new Date(o.value.expires_at).getTime() - Date.now()
  if (ms <= 0) return ''
  const h = Math.floor(ms / 3_600_000)
  const m = Math.floor((ms % 3_600_000) / 60_000)
  return h ? t('checkout.hoursMinutes', { h, m }) : t('checkout.minutes', { m })
})
</script>

<template>
  <div v-if="doc" class="mx-auto max-w-xl px-4 py-8">
    <div class="flex items-center gap-3">
      <ProductThumb :src="doc.shop.logo_url" :name="doc.shop.name" class="size-12" rounded="rounded-2xl" />
      <div>
        <div class="text-lg font-extrabold">{{ doc.shop.name }}</div>
        <i18n-t keypath="checkout.orderVia" tag="div" class="text-sm text-muted"><template #number><span class="font-mono font-bold text-ink">{{ o.number }}</span></template><template #provider><span class="capitalize">{{ doc.customer.provider }}</span></template></i18n-t>
      </div>
    </div>

    <div class="mt-5 rounded-2xl px-4 py-3 text-sm font-semibold" :class="['cancelled', 'expired'].includes(o.status) ? 'bg-brand-50 text-brand-700' : ['paid', 'shipped', 'completed'].includes(o.status) ? 'bg-mint-500/10 text-mint-500' : 'bg-sun-400/15 text-amber-800'">
      {{ statusText }}
      <ClientOnly><span v-if="['open', 'confirmed'].includes(o.status) && remaining" class="block text-xs font-normal">{{ $t('checkout.reserved', { time: remaining }) }}</span></ClientOnly>
      <span v-if="['shipped', 'completed'].includes(o.status) && o.carrier_code" class="block text-xs font-normal">{{ $t('delivery.shippedWith', { courier: courierName }) }}</span>
      <span v-if="['shipped', 'completed'].includes(o.status) && o.tracking_no" class="block text-xs">
        <span class="font-mono">{{ $t('checkout.tracking', { no: o.tracking_no }) }}</span>
        <a v-if="trackUrl" :href="trackUrl" target="_blank" rel="noopener" class="ml-2 underline">{{ $t('delivery.track') }}</a>
      </span>
      <span v-if="isCod && o.cod_status === 'pending' && !['cancelled', 'expired'].includes(o.status)" class="block text-xs">{{ $t('delivery.codCash', { amount: money(o.cod_amount_cents, cur) }) }}</span>
    </div>

    <ol v-if="!['cancelled', 'expired'].includes(o.status)" class="card mt-5 p-5" :aria-label="$t('checkout.track.title')">
      <li v-for="(s, i) in timeline" :key="s.key" class="relative flex gap-3" :class="i < timeline.length - 1 ? 'pb-4' : ''">
        <span v-if="i < timeline.length - 1" class="absolute left-[9px] top-6 h-[calc(100%-1.25rem)] w-0.5" :class="timeline[i + 1]!.done ? 'bg-mint-500' : 'bg-line'" />
        <span class="relative mt-0.5 grid size-5 shrink-0 place-items-center rounded-full text-[11px] font-bold" :class="s.done ? 'bg-mint-500 text-white' : 'border-2 border-line bg-paper'">{{ s.done ? '✓' : '' }}</span>
        <div>
          <div class="text-sm font-semibold" :class="s.done ? 'text-ink' : 'text-muted'">{{ $t(`checkout.track.${s.key}`) }}</div>
          <div v-if="s.done && s.at" class="text-xs text-muted"><ClientOnly>{{ fmtDateTime(s.at) }}</ClientOnly></div>
          <div v-if="s.key === 'shipped' && s.done && o.tracking_no" class="text-xs"><span class="font-mono">{{ o.tracking_no }}</span><a v-if="trackUrl" :href="trackUrl" target="_blank" rel="noopener" class="ml-2 font-semibold text-brand-700 underline">{{ $t('delivery.track') }}</a></div>
        </div>
      </li>
    </ol>

    <div class="card mt-5 overflow-hidden">
      <ul class="divide-y divide-line/70">
        <li v-for="it in doc.items" :key="it.id" class="flex items-center gap-3 p-4">
          <span class="rounded-lg bg-ink px-2 py-1 font-mono text-xs font-bold text-paper">{{ it.code }}</span>
          <div class="min-w-0 flex-1"><div class="truncate font-semibold">{{ it.name }}</div><div class="text-xs text-muted">{{ it.qty }} × {{ money(it.unit_price_cents, cur) }}</div></div>
          <div class="font-bold tabular-nums">{{ money(it.qty * it.unit_price_cents, cur) }}</div>
        </li>
      </ul>
      <dl class="space-y-1 border-t border-line bg-paper p-4 text-sm">
        <div class="flex justify-between"><dt>{{ $t('common.subtotal') }}</dt><dd class="tabular-nums">{{ money(o.subtotal_cents, cur) }}</dd></div>
        <div class="flex justify-between"><dt>{{ $t('common.shipping') }}</dt><dd class="tabular-nums">{{ shipLine }}</dd></div>
        <div v-if="codFeeLine" class="flex justify-between"><dt>{{ $t('delivery.codFee') }}</dt><dd class="tabular-nums">{{ money(codFeeLine, cur) }}</dd></div>
        <div v-if="discountLine" class="flex justify-between text-mint-500"><dt>{{ $t('common.discount') }}</dt><dd class="tabular-nums">−{{ money(discountLine, cur) }}</dd></div>
        <div class="flex justify-between text-lg font-extrabold"><dt>{{ $t('common.total') }}</dt><dd class="tabular-nums">{{ money(totalLine, cur) }}</dd></div>
        <p v-if="!editing && o.carrier_code && o.fee_payer === 'destination'" class="text-xs text-muted">{{ $t('delivery.destinationShort') }}</p>
      </dl>
    </div>

    <template v-if="['open', 'confirmed'].includes(o.status)">
      <form v-if="editing" class="card mt-5 space-y-3 p-5" @submit.prevent="confirm">
        <div class="font-bold">{{ $t('checkout.deliveryDetails') }}</div>
        <UInput v-model="f.name" required :placeholder="$t('checkout.fullName')" autocomplete="name" />
        <UInput v-model="f.phone" required :placeholder="$t('checkout.phoneNumber')" inputmode="tel" autocomplete="tel" />
        <UTextarea v-model="f.address" required :rows="3" :placeholder="$t('checkout.addressPlaceholder')" autocomplete="street-address" />
        <UInput v-model="f.note" :placeholder="$t('checkout.notePlaceholder')" />
        <div v-if="hasOptions" class="space-y-2 pt-1">
          <div class="font-bold">{{ $t('delivery.title') }}</div>
          <DeliveryPicker v-model="choice" :options="shippingOptions" :currency="cur" :subtotal-cents="o.subtotal_cents" />
          <p v-if="choice?.cod" class="rounded-xl bg-sun-400/15 p-2.5 text-xs text-amber-900">{{ $t('delivery.codCash', { amount: money(totalLine, cur) }) }}</p>
        </div>
        <ModuleSlot name="checkout.promo" :token="token" :phone="f.phone" />
        <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
        <UButton type="submit" class="w-full py-3" :disabled="busy">{{ $t('checkout.confirmOrder') }}</UButton>
      </form>
      <div v-else class="card mt-5 p-5 text-sm">
        <div class="flex items-center justify-between"><div class="font-bold">{{ $t('checkout.deliverTo') }}</div><button class="text-xs font-semibold text-brand-600 underline" @click="editing = true">{{ $t('common.edit') }}</button></div>
        <div class="mt-1 font-semibold">{{ o.ship_name }} · {{ o.ship_phone }}</div>
        <div class="whitespace-pre-line text-ink/80">{{ o.ship_address }}</div>
        <div v-if="o.carrier_code" class="mt-2 text-xs text-muted">{{ courierName }} · {{ deliveryTypeText }}<template v-if="isCod"> · {{ $t('common.pay.cod') }}</template></div>
      </div>

      <div v-if="o.status === 'confirmed' && isCod && !editing" class="card mt-5 space-y-1 p-5">
        <div class="font-bold">{{ $t('checkout.payment') }}</div>
        <p class="text-sm">{{ $t('delivery.codCash', { amount: money(o.cod_amount_cents, cur) }) }}</p>
        <p v-if="o.fee_payer === 'destination'" class="text-xs text-muted">{{ $t('delivery.destinationShort') }}</p>
      </div>
      <div v-else-if="o.status === 'confirmed' && !isCod" class="card mt-5 space-y-3 p-5">
        <div class="font-bold">{{ $t('checkout.payment') }}</div>
        <div v-if="doc.shop.payment_instructions" class="whitespace-pre-line rounded-xl bg-paper p-3 text-sm">{{ doc.shop.payment_instructions }}</div>
        <p v-else class="text-sm text-muted">{{ $t('checkout.contactForPayment') }}</p>
        <form class="space-y-2" @submit.prevent="sendPayment">
          <div class="flex gap-2">
            <select v-model="pay.method" class="input w-36"><option value="transfer">{{ $t('common.pay.transfer') }}</option><option value="promptpay">{{ $t('common.pay.promptpay') }}</option></select>
            <UInput v-model="pay.reference" :placeholder="$t('checkout.refPlaceholder')" />
          </div>
          <p v-if="error && !editing" class="text-sm text-brand-700">{{ error }}</p>
          <UButton color="neutral" type="submit" class="w-full" :disabled="busy">{{ o.payment_ref ? $t('checkout.updatePayment') : $t('checkout.iPaid') }}</UButton>
          <p v-if="o.payment_ref" class="text-center text-xs text-mint-500">{{ $t('checkout.thanks', { ref: o.payment_ref }) }}</p>
        </form>
      </div>
    </template>
    <p class="mt-8 text-center text-xs text-muted">{{ $t('checkout.poweredBy') }} · <NuxtLink :to="`/s/${doc.shop.slug}`" class="underline">{{ $t('checkout.visit', { shop: doc.shop.name }) }}</NuxtLink></p>
  </div>
</template>
