<script setup lang="ts">
import type { SocialOrderDoc } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const route = useRoute()
const { t } = useI18n()
const { print, open } = usePrint()
const { data: doc, refresh } = await useAsyncData(`social-order:${route.params.id}`, () => api<SocialOrderDoc>(`/social/orders/${route.params.id}`))
const o = computed(() => doc.value!.order)
const cur = computed(() => o.value.currency)
const error = ref('')
const tracking = ref('')
const copied = ref(false)

async function setStatus(status: string, extra: Record<string, string> = {}) {
  error.value = ''
  try {
    doc.value = await api<SocialOrderDoc>(`/social/orders/${route.params.id}/status`, { method: 'POST', body: { status, ...extra } })
  } catch (e) {
    error.value = apiError(e)
  }
}
async function setQty(productId: string | null, qty: number) {
  if (!productId) return
  error.value = ''
  try {
    doc.value = await api<SocialOrderDoc>(`/social/orders/${route.params.id}/items`, { method: 'POST', body: { product_id: productId, qty } })
  } catch (e) {
    error.value = apiError(e)
    refresh()
  }
}
async function copyLink() {
  await navigator.clipboard?.writeText(doc.value!.checkout_url!)
  copied.value = true
  setTimeout(() => (copied.value = false), 1500)
}
const { carriers, carrierName, trackingLink } = useCarriers()
const cod = computed(() => o.value.payment_method === 'cod')
const canShip = computed(() => o.value.status === 'paid' || (cod.value && o.value.status === 'confirmed'))
const shipCarrier = ref('')
watch(() => doc.value?.order.carrier_code, (c) => (shipCarrier.value = c ?? ''), { immediate: true })
const trackLink = computed(() => trackingLink(o.value.carrier_code, o.value.tracking_no))
const editable = computed(() => ['open', 'confirmed'].includes(o.value.status))
const steps = ['open', 'confirmed', 'paid', 'shipped', 'completed']
const stepLabel = (s: string) => t(`social.order.steps.${s}`)
</script>

<template>
  <div v-if="doc">
    <NuxtLink to="/dashboard/social/orders" class="text-sm text-muted hover:text-ink">{{ $t('social.order.back') }}</NuxtLink>
    <div class="mt-2 flex flex-wrap items-center gap-3">
      <h1 class="page-title font-mono">{{ o.number }}</h1>
      <StatusBadge :status="o.status" />
      <div class="ml-auto flex gap-2">
        <UButton color="neutral" variant="soft" size="sm" type="submit" @click="open(`/print/social/${o.id}`)">{{ $t('social.order.invoice') }}</UButton>
        <UButton color="neutral" variant="soft" size="sm" type="submit" @click="print(`/print/social/${o.id}?type=packing`)">{{ $t('social.order.packingSlip') }}</UButton>
        <UButton color="neutral" variant="soft" size="sm" type="submit" v-if="o.carrier_code || cod" @click="open(`/print/label/social/${o.id}`)">{{ $t('ship.printLabel') }}</UButton>
      </div>
    </div>

    <ol v-if="!['cancelled', 'expired'].includes(o.status)" class="mt-5 flex gap-2">
      <li v-for="s in steps" :key="s" class="flex-1">
        <div class="h-1.5 rounded-full" :class="steps.indexOf(o.status) >= steps.indexOf(s) ? 'bg-brand-500' : 'bg-line'" />
        <div class="mt-1 text-[11px] font-semibold" :class="steps.indexOf(o.status) >= steps.indexOf(s) ? 'text-ink' : 'text-muted'">{{ stepLabel(s) }}</div>
      </li>
    </ol>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>

    <div class="mt-6 grid gap-6 lg:grid-cols-[1fr_340px]">
      <div class="space-y-6">
        <div class="card overflow-hidden">
          <table class="table">
            <thead><tr><th>{{ $t('social.order.code') }}</th><th>{{ $t('social.order.item') }}</th><th class="w-40">{{ $t('common.qty') }}</th><th class="text-right">{{ $t('social.order.amount') }}</th></tr></thead>
            <tbody>
              <tr v-for="it in doc.items" :key="it.id">
                <td class="font-mono font-bold">{{ it.code }}</td>
                <td>{{ it.name }}<div class="text-xs text-muted">{{ money(it.unit_price_cents, cur) }}</div></td>
                <td>
                  <div v-if="editable" class="flex items-center gap-1">
                    <UButton color="neutral" variant="soft" size="sm" type="submit" class="px-2.5" @click="setQty(it.product_id, it.qty - 1)">−</UButton>
                    <span class="w-8 text-center font-bold">{{ it.qty }}</span>
                    <UButton color="neutral" variant="soft" size="sm" type="submit" class="px-2.5" @click="setQty(it.product_id, it.qty + 1)">+</UButton>
                  </div>
                  <span v-else class="font-bold">{{ it.qty }}</span>
                </td>
                <td class="text-right font-semibold tabular-nums">{{ money(it.qty * it.unit_price_cents, cur) }}</td>
              </tr>
            </tbody>
          </table>
          <dl class="space-y-1 border-t border-line p-4 text-sm">
            <div class="flex justify-between"><dt class="text-muted">{{ $t('common.subtotal') }}</dt><dd class="tabular-nums">{{ money(o.subtotal_cents, cur) }}</dd></div>
            <div class="flex justify-between"><dt class="text-muted">{{ $t('common.shipping') }}</dt><dd class="tabular-nums">{{ o.fee_payer === 'buyer' || !o.carrier_code ? money(o.shipping_cents, cur) : $t(`ship.payerShort.${o.fee_payer}`) }}</dd></div>
            <div v-if="o.cod_fee_cents" class="flex justify-between"><dt class="text-muted">{{ $t('ship.codFee') }}</dt><dd class="tabular-nums">{{ money(o.cod_fee_cents, cur) }}</dd></div>
            <div class="flex justify-between text-lg font-extrabold"><dt>{{ $t('common.total') }}</dt><dd class="tabular-nums">{{ money(o.total_cents, cur) }}</dd></div>
          </dl>
        </div>

        <div class="card p-5">
          <div class="font-bold">{{ $t('social.order.conversation') }}</div>
          <ul class="mt-3 space-y-2 text-sm">
            <li v-for="(c, i) in doc.comments" :key="i" class="flex items-start gap-2">
              <SocialProviderBadge :provider="doc.customer.provider" />
              <div><span class="font-semibold">{{ doc.customer.name }}</span> <span class="text-xs text-muted">{{ ago(c.created_at) }}</span><div>{{ c.message }}</div></div>
              <StatusBadge class="ml-auto" :status="c.result" />
            </li>
          </ul>
        </div>
      </div>

      <aside class="space-y-4">
        <div class="card space-y-3 p-5">
          <div class="font-bold">{{ $t('social.order.nextStep') }}</div>
          <template v-if="o.status === 'open'">
            <p class="text-sm text-muted">{{ $t('social.order.waitingAddress', { name: doc.customer.name || $t('social.order.theCustomer'), date: fmtDateTime(o.expires_at) }) }}</p>
            <UButton color="neutral" variant="soft" type="submit" class="w-full" @click="copyLink">{{ copied ? $t('social.order.copied') : $t('social.order.copyLink') }}</UButton>
            <UButton type="submit" class="w-full" @click="setStatus('paid')">{{ $t('social.order.markPaid') }}</UButton>
          </template>
          <form v-else-if="canShip" class="space-y-2" @submit.prevent="setStatus('shipped', { tracking_no: tracking, ...(shipCarrier ? { carrier_code: shipCarrier } : {}) })">
            <p v-if="cod && o.status === 'confirmed'" class="text-sm">{{ $t('ship.codShipHint', { amount: money(o.cod_amount_cents, cur) }) }}</p>
            <select v-model="shipCarrier" class="input" :aria-label="$t('ship.chooseCourier')">
              <option value="">{{ $t('ship.chooseCourier') }}</option>
              <option v-for="c in carriers" :key="c.code" :value="c.code">{{ carrierName(c.code) }}</option>
            </select>
            <UInput v-model="tracking" maxlength="60" :placeholder="$t('social.order.trackingPlaceholder')" />
            <UButton type="submit" class="w-full">{{ $t('social.order.markShipped') }}</UButton>
          </form>
          <template v-else-if="o.status === 'confirmed'">
            <p class="text-sm"><template v-if="o.payment_ref">{{ $t('social.order.customerReports') }} <b>{{ o.payment_method }} · {{ o.payment_ref }}</b></template><template v-else>{{ $t('social.order.awaitingPayment') }}</template></p>
            <UButton type="submit" class="w-full" @click="setStatus('paid')">{{ $t('social.order.paymentReceived') }}</UButton>
          </template>
          <template v-else-if="o.status === 'shipped'">
            <p class="text-sm">{{ $t('social.order.tracking') }}
              <a v-if="trackLink" :href="trackLink" target="_blank" rel="noopener" class="font-mono font-bold text-brand-700 hover:underline">{{ o.tracking_no }}</a>
              <b v-else class="font-mono">{{ o.tracking_no || '—' }}</b>
            </p>
            <UButton type="submit" class="w-full" @click="setStatus('completed')">{{ cod ? $t('ship.deliveredCod') : $t('social.order.markDelivered') }}</UButton>
            <NuxtLink v-if="cod" to="/dashboard/cod" class="block text-center text-xs font-semibold text-brand-700 hover:underline">{{ $t('ship.codLedgerLink') }}</NuxtLink>
          </template>
          <p v-else class="text-sm text-muted">{{ $t('social.order.orderIs', { status: $te(`common.status.${o.status}`) ? $t(`common.status.${o.status}`) : o.status }) }}</p>
          <UButton color="neutral" variant="ghost" type="submit" v-if="['open', 'confirmed', 'paid'].includes(o.status)" class="w-full bg-brand-50 text-brand-700 hover:bg-brand-100" @click="setStatus('cancelled')">{{ $t('social.order.cancelRelease') }}</UButton>
        </div>
        <div v-if="o.carrier_code || cod" class="card space-y-2 p-5 text-sm">
          <div class="font-bold">{{ $t('ship.courier') }}</div>
          <div v-if="o.carrier_code" class="font-semibold">🚚 {{ carrierName(o.carrier_code) }} · {{ $t(`ship.delivery.${o.delivery_type}`) }}</div>
          <div v-if="o.carrier_code" class="text-muted">{{ $t(`ship.payerShort.${o.fee_payer}`) }}</div>
          <div v-if="cod" class="flex flex-wrap items-center gap-2">
            <span class="chip bg-sun-400/25 text-amber-800">{{ $t('ship.codBadge', { amount: money(o.cod_amount_cents, cur) }) }}</span>
            <StatusBadge v-if="o.cod_status !== 'none'" :status="o.cod_status" />
          </div>
          <div v-if="o.cod_remit_ref" class="text-xs text-muted">{{ $t('ship.cod.remitRefShown', { ref: o.cod_remit_ref }) }}</div>
          <div v-if="o.shipped_at" class="text-xs text-muted">{{ $t('ship.shippedOn', { date: fmtDateTime(o.shipped_at) }) }}</div>
        </div>
        <div class="card space-y-1 p-5 text-sm">
          <div class="flex items-center gap-2 font-bold"><SocialProviderBadge :provider="doc.customer.provider" size="md" /> {{ doc.customer.name || $t('common.customer') }}</div>
          <div class="text-xs text-muted">{{ $t('social.order.customerId', { id: doc.customer.external_user_id }) }}</div>
          <template v-if="o.ship_address">
            <div class="mt-3 font-semibold">{{ o.ship_name }} · {{ o.ship_phone }}</div>
            <div class="whitespace-pre-line text-ink/80">{{ o.ship_address }}</div>
            <div v-if="o.customer_note" class="mt-2 rounded-lg bg-paper p-2 text-xs">“{{ o.customer_note }}”</div>
          </template>
          <p v-else class="mt-2 text-muted">{{ $t('social.order.noAddress') }}</p>
        </div>
      </aside>
    </div>
  </div>
</template>
