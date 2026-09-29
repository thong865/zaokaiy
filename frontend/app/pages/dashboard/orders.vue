<script setup lang="ts">
import type { Order } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId } = useShop()
const { t } = useI18n()
const role = ref<'supplier' | 'seller'>('supplier')
const status = ref('')
const { data: orders, refresh } = await useAsyncData(
  'shop-orders',
  () => api<Order[]>(`/shops/${shopId.value}/sales`, { query: { role: role.value, status: status.value || undefined } }),
  { watch: [shopId, role, status], default: () => [] },
)
const error = ref('')
const { print, open } = usePrint()
async function move(o: Order, next: string) {
  error.value = ''
  try {
    await api(`/shops/${shopId.value}/orders/${o.id}/status`, { method: 'POST', body: { status: next } })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
const next = computed<Record<string, { to: string; label: string }[]>>(() => ({
  pending: [{ to: 'cancelled', label: t('common.cancel') }],
  paid: [{ to: 'cancelled', label: t('common.cancel') }],
  shipped: [{ to: 'completed', label: t('dash.orders.markDelivered') }],
}))
const nextFor = (o: Order) =>
  (next.value[o.status] ?? []).map((n) => (n.to === 'completed' && o.payment_method === 'cod' ? { ...n, label: t('ship.deliveredCod') } : n))

// ---- delivery (courier, tracking, COD)
const { carriers, carrierName, trackingLink } = useCarriers()
const canShip = (o: Order) => o.status === 'paid' || (o.status === 'pending' && o.payment_method === 'cod')
const shipForms = reactive<Record<string, { carrier: string; tracking: string }>>({})
const shipForm = (o: Order) => (shipForms[o.id] ??= { carrier: o.carrier_code ?? '', tracking: o.tracking_no ?? '' })
// Create the forms up front (not during render).
watch(orders, (list) => list.forEach((o) => canShip(o) && shipForm(o)), { immediate: true })
async function ship(o: Order) {
  const f = shipForm(o)
  error.value = ''
  try {
    await api(`/shops/${shopId.value}/orders/${o.id}/status`, {
      method: 'POST',
      body: { status: 'shipped', carrier_code: f.carrier || undefined, tracking_no: f.tracking.trim() || undefined },
    })
    delete shipForms[o.id]
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
const addrOf = (o: Order) => {
  const a = o.shipping_address ?? {}
  return a.address || [a.line1, a.city, a.postcode].filter(Boolean).join(' ')
}
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('dash.orders.title') }}</h1>
    <div class="mt-5 flex flex-wrap items-center gap-3">
      <div class="flex rounded-2xl bg-surface p-1 shadow-card text-sm font-semibold">
        <button class="rounded-xl px-4 py-1.5" :class="role === 'supplier' && 'bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]'" @click="role = 'supplier'">{{ $t('dash.orders.mine') }}</button>
        <button class="rounded-xl px-4 py-1.5" :class="role === 'seller' && 'bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]'" @click="role = 'seller'">{{ $t('dash.orders.asSellStaff') }}</button>
      </div>
      <select v-model="status" class="input w-40">
        <option value="">{{ $t('dash.orders.allStatuses') }}</option>
        <option v-for="s in ['pending', 'paid', 'shipped', 'completed', 'cancelled']" :key="s" :value="s" class="capitalize">{{ $t(`common.status.${s}`) }}</option>
      </select>
    </div>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>
    <div v-if="orders.length" class="mt-5 space-y-3">
      <div v-for="o in orders" :key="o.id" class="card p-5">
        <div class="flex flex-wrap items-center gap-3">
          <div class="font-bold">#{{ shortId(o.id) }}</div>
          <StatusBadge :status="o.status" />
          <span class="text-xs text-muted">{{ ago(o.created_at) }}</span>
          <span class="ml-auto font-bold">{{ money(o.grand_total_cents ?? o.total_cents, o.currency) }}</span>
          <template v-if="role === 'supplier'">
            <UButton color="neutral" variant="soft" size="sm" type="submit" @click="open(`/print/order/${o.id}`)">{{ $t('dash.orders.invoice') }}</UButton>
            <UButton color="neutral" variant="soft" size="sm" type="submit" v-if="o.carrier_code || o.payment_method === 'cod'" @click="open(`/print/label/order/${o.id}`)">{{ $t('ship.printLabel') }}</UButton>
            <UButton color="neutral" variant="soft" size="sm" type="submit" @click="print(`/print/order/${o.id}?type=packing`)">{{ $t('dash.orders.packingSlip') }}</UButton>
          </template>
        </div>
        <div class="mt-3 grid gap-4 text-sm md:grid-cols-[1fr_260px]">
          <ul class="space-y-1">
            <li v-for="it in o.items" :key="it.id" class="flex flex-wrap justify-between gap-2">
              <span>{{ it.product_name }} × {{ it.qty }}</span>
              <span v-if="it.seller_shop_id" class="chip bg-brand-50 text-brand-700">{{ $t('dash.orders.sellStaffChip', { pct: pct(it.commission_bps), amount: money(it.commission_cents, o.currency) }) }}</span>
            </li>
          </ul>
          <div v-if="role === 'supplier'" class="text-xs text-muted">
            <div class="font-semibold text-ink">{{ o.shipping_address?.name }}</div>
            <span class="whitespace-pre-line">{{ addrOf(o) }}</span><br>{{ o.shipping_address?.phone }}
          </div>
        </div>
        <div v-if="o.carrier_code || o.payment_method === 'cod' || o.tracking_no" class="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 rounded-xl bg-paper px-3 py-2 text-xs">
          <span v-if="o.carrier_code" class="font-bold text-ink">🚚 {{ carrierName(o.carrier_code) }}</span>
          <span v-if="o.carrier_code">{{ $t(`ship.delivery.${o.delivery_type}`) }}</span>
          <span v-if="o.delivery_type === 'branch' && o.shipping_address?.branch">{{ $t('ship.pickupBranch', { branch: o.shipping_address.branch }) }}</span>
          <span v-if="o.carrier_code" class="text-muted">{{ $t(`ship.payerShort.${o.fee_payer}`) }}<template v-if="o.fee_payer !== 'seller' && o.shipping_fee_cents"> · {{ money(o.shipping_fee_cents, o.currency) }}</template></span>
          <template v-if="o.payment_method === 'cod'">
            <span class="chip bg-sun-400/25 text-amber-800">{{ $t('ship.codBadge', { amount: money(o.cod_amount_cents, o.currency) }) }}</span>
            <StatusBadge v-if="o.cod_status !== 'none'" :status="o.cod_status" />
          </template>
          <span v-if="o.tracking_no">{{ $t('ship.tracking') }}:
            <a v-if="trackingLink(o.carrier_code, o.tracking_no)" :href="trackingLink(o.carrier_code, o.tracking_no)!" target="_blank" rel="noopener" class="font-mono font-semibold text-brand-700 hover:underline">{{ o.tracking_no }}</a>
            <b v-else class="font-mono">{{ o.tracking_no }}</b>
          </span>
          <span v-if="o.shipped_at" class="text-muted">{{ $t('ship.shippedOn', { date: fmtDate(o.shipped_at) }) }}</span>
        </div>
        <div v-if="role === 'supplier' && (canShip(o) || nextFor(o).length)" class="mt-4 flex flex-wrap items-end gap-2 border-t border-line pt-4">
          <form v-if="canShip(o) && shipForms[o.id]" class="flex flex-1 flex-wrap items-end gap-2" @submit.prevent="ship(o)">
            <select v-model="shipForms[o.id]!.carrier" class="input w-44 py-1.5" :aria-label="$t('ship.chooseCourier')">
              <option value="">{{ $t('ship.chooseCourier') }}</option>
              <option v-for="c in carriers" :key="c.code" :value="c.code">{{ carrierName(c.code) }}</option>
            </select>
            <UInput size="md" v-model="shipForms[o.id]!.tracking" class="w-52 flex-1" maxlength="60" :placeholder="$t('ship.trackingPlaceholder')" :aria-label="$t('ship.tracking')" />
            <UButton size="sm" type="submit">{{ $t('ship.markShipped') }}</UButton>
          </form>
          <button v-for="n in nextFor(o)" :key="n.to" :class="n.to === 'cancelled' ? 'btn-ghost btn-sm' : 'btn-primary btn-sm'" @click="move(o, n.to)">{{ n.label }}</button>
        </div>
      </div>
    </div>
    <EmptyState v-else class="mt-5" :title="$t('dash.orders.emptyTitle')" :text="role === 'seller' ? $t('dash.orders.emptySeller') : $t('dash.orders.emptySupplier')" />
  </div>
</template>
