<script setup lang="ts">
import type { Order } from '~/utils/types'

definePageMeta({ middleware: 'auth' })
const api = useApi()
const { data: orders, refresh } = await useAsyncData('my-orders', () => api<Order[]>('/orders'), { default: () => [] })
const error = ref('')
async function act(o: Order, action: 'pay' | 'cancel') {
  error.value = ''
  try {
    await api(`/orders/${o.id}/${action}`, { method: 'POST' })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
const steps = ['pending', 'paid', 'shipped', 'completed']
/** COD orders skip the online payment step: the courier collects the cash on delivery. */
const codSteps = ['pending', 'shipped', 'completed']
const stepsOf = (o: Order) => (o.payment_method === 'cod' ? codSteps : steps)
const { t } = useI18n()
const { carrierName, trackingLink } = useCarriers()
const rows = computed(() =>
  orders.value.map((o) => ({
    o,
    isCod: o.payment_method === 'cod',
    courier: o.carrier_code ? carrierName(o.carrier_code) : '',
    type: o.carrier_code ? t(`delivery.${o.delivery_type}`) : '',
    link: o.tracking_no ? trackingLink(o.carrier_code, o.tracking_no) : null,
    total: o.grand_total_cents || o.total_cents,
  })),
)
useHead({ title: () => t('common.nav.myOrders') + ' · zaokaiy' })
</script>

<template>
  <div class="mx-auto max-w-4xl px-4 pt-10">
    <h1 class="page-title text-3xl">{{ $t('common.nav.myOrders') }}</h1>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>
    <div v-if="rows.length" class="mt-6 space-y-4">
      <div v-for="{ o, isCod, courier, type, link, total } in rows" :key="o.id" class="card p-5">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div>
            <div class="font-bold">#{{ shortId(o.id) }}</div>
            <div class="text-xs text-muted">{{ fmtDateTime(o.created_at) }}</div>
          </div>
          <div class="flex flex-wrap items-center gap-1.5">
            <StatusBadge v-if="isCod && o.status === 'pending'" status="confirmed" :label="$t('delivery.codConfirmed')" />
            <StatusBadge v-else :status="o.status" />
          </div>
        </div>
        <div v-if="o.status !== 'cancelled'" class="mt-4 flex gap-1">
          <div v-for="s in stepsOf(o)" :key="s" class="h-1.5 flex-1 rounded-full" :class="stepsOf(o).indexOf(o.status) >= stepsOf(o).indexOf(s) ? 'bg-brand-500' : 'bg-line'" />
        </div>
        <ul class="mt-4 space-y-1 text-sm">
          <li v-for="it in o.items" :key="it.id" class="flex justify-between">
            <span>{{ it.product_name }} × {{ it.qty }}</span>
            <span>{{ money(it.unit_price_cents * it.qty, o.currency) }}</span>
          </li>
          <li v-if="o.carrier_code" class="flex justify-between text-muted">
            <span>{{ $t('delivery.shipping') }}</span>
            <span>{{ o.fee_payer === 'seller' ? $t('delivery.free') : o.fee_payer === 'destination' ? $t('delivery.payAtPickup') : money(o.shipping_fee_cents, o.currency) }}</span>
          </li>
          <li v-if="o.cod_fee_cents" class="flex justify-between text-muted">
            <span>{{ $t('delivery.codFee') }}</span>
            <span>{{ money(o.cod_fee_cents, o.currency) }}</span>
          </li>
        </ul>
        <div v-if="o.carrier_code" class="mt-3 space-y-1 rounded-xl bg-paper p-3 text-xs">
          <div><span class="font-semibold">{{ courier }}</span> · {{ type }}</div>
          <div v-if="o.tracking_no" class="flex flex-wrap items-center gap-2">
            <span>{{ $t('delivery.trackingNo') }}: <span class="font-mono font-semibold">{{ o.tracking_no }}</span></span>
            <a v-if="link" :href="link" target="_blank" rel="noopener" class="font-semibold text-brand-600 underline">{{ $t('delivery.track') }}</a>
          </div>
          <div v-if="o.fee_payer === 'destination'" class="text-muted">{{ $t('delivery.destinationNote', { fee: money(o.shipping_fee_cents, o.currency) }) }}</div>
          <div v-if="isCod" class="flex flex-wrap items-center gap-2">
            <span v-if="o.cod_status === 'pending'" class="font-semibold text-amber-800">{{ $t('delivery.codToCourier', { amount: money(o.cod_amount_cents, o.currency) }) }}</span>
            <span v-else>{{ $t('delivery.codAmount', { amount: money(o.cod_amount_cents, o.currency) }) }}</span>
            <StatusBadge v-if="o.cod_status !== 'none'" :status="o.cod_status" :label="$t(`delivery.codStatus.${o.cod_status}`)" />
          </div>
        </div>
        <div class="mt-4 flex items-center justify-between border-t border-line pt-4">
          <span class="font-bold">{{ money(total, o.currency) }}</span>
          <div class="flex gap-2">
            <UButton color="neutral" variant="soft" size="sm" v-if="o.status !== 'cancelled'" :to="`/print/order/${o.id}`" target="_blank">{{ $t('store.orders.receipt') }}</UButton>
            <UButton size="sm" type="submit" v-if="o.status === 'pending' && !isCod" @click="act(o, 'pay')">{{ $t('store.orders.payDemo') }}</UButton>
            <UButton color="neutral" variant="soft" size="sm" type="submit" v-if="['pending', 'paid'].includes(o.status)" @click="act(o, 'cancel')">{{ $t('store.orders.cancel') }}</UButton>
          </div>
        </div>
      </div>
    </div>
    <EmptyState v-else class="mt-6" :title="$t('store.orders.emptyTitle')"><UButton to="/">{{ $t('store.orders.startShopping') }}</UButton></EmptyState>
  </div>
</template>
