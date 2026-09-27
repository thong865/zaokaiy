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
  paid: [{ to: 'shipped', label: t('dash.orders.markShipped') }, { to: 'cancelled', label: t('common.cancel') }],
  shipped: [{ to: 'completed', label: t('dash.orders.markDelivered') }],
}))
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('dash.orders.title') }}</h1>
    <div class="mt-5 flex flex-wrap items-center gap-3">
      <div class="flex rounded-full border border-line bg-white p-1 text-sm font-semibold">
        <button class="rounded-full px-4 py-1.5" :class="role === 'supplier' && 'bg-ink text-white'" @click="role = 'supplier'">{{ $t('dash.orders.mine') }}</button>
        <button class="rounded-full px-4 py-1.5" :class="role === 'seller' && 'bg-ink text-white'" @click="role = 'seller'">{{ $t('dash.orders.asSellStaff') }}</button>
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
          <span class="ml-auto font-bold">{{ money(o.total_cents, o.currency) }}</span>
          <template v-if="role === 'supplier'">
            <button class="btn-ghost btn-sm" @click="open(`/print/order/${o.id}`)">{{ $t('dash.orders.invoice') }}</button>
            <button class="btn-ghost btn-sm" @click="print(`/print/order/${o.id}?type=packing`)">{{ $t('dash.orders.packingSlip') }}</button>
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
            {{ o.shipping_address?.line1 }} {{ o.shipping_address?.city }} {{ o.shipping_address?.postcode }}<br>{{ o.shipping_address?.phone }}
          </div>
        </div>
        <div v-if="role === 'supplier' && next[o.status]" class="mt-4 flex gap-2 border-t border-line pt-4">
          <button v-for="n in next[o.status]" :key="n.to" :class="n.to === 'cancelled' ? 'btn-ghost btn-sm' : 'btn-primary btn-sm'" @click="move(o, n.to)">{{ n.label }}</button>
        </div>
      </div>
    </div>
    <EmptyState v-else class="mt-5" :title="$t('dash.orders.emptyTitle')" :text="role === 'seller' ? $t('dash.orders.emptySeller') : $t('dash.orders.emptySupplier')" />
  </div>
</template>
