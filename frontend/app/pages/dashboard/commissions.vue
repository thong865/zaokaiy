<script setup lang="ts">
definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId, shop } = useShop()
type Summary = Record<'pending' | 'approved' | 'paid' | 'void', number>
interface Earned { summary: Summary; items: { id: string; order_id: string; product_name: string; supplier_name: string; amount_cents: number; status: string; created_at: string }[] }
interface Payable { summary: Summary; items: { id: string; order_id: string; product_name: string; reseller_name: string; amount_cents: number; status: string; created_at: string }[] }
const tab = ref<'earned' | 'payable'>('earned')
const { data: earned } = await useAsyncData('comm-earned', () => api<Earned>(`/shops/${shopId.value}/commissions`), { watch: [shopId] })
const { data: payable, refresh } = await useAsyncData('comm-payable', () => api<Payable>(`/shops/${shopId.value}/commissions/payable`), { watch: [shopId] })
const cur = computed(() => shop.value?.currency ?? 'THB')
const summary = computed(() => (tab.value === 'earned' ? earned.value?.summary : payable.value?.summary))
const error = ref('')
async function pay(id: string) {
  error.value = ''
  try {
    await api(`/commissions/${id}/pay`, { method: 'POST' })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('network.comm.title') }}</h1>
    <i18n-t keypath="network.comm.intro" tag="p" class="text-sm text-muted"><template #pending><b>{{ $t('network.comm.introPending') }}</b></template><template #approved><b>{{ $t('network.comm.introApproved') }}</b></template><template #paid><b>{{ $t('network.comm.introPaid') }}</b></template></i18n-t>
    <div class="mt-5 flex rounded-2xl bg-surface p-1 shadow-card text-sm font-semibold w-fit">
      <button class="rounded-xl px-4 py-1.5" :class="tab === 'earned' && 'bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]'" @click="tab = 'earned'">{{ $t('network.comm.earned') }}</button>
      <button class="rounded-xl px-4 py-1.5" :class="tab === 'payable' && 'bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]'" @click="tab = 'payable'">{{ $t('network.comm.payable') }}</button>
    </div>
    <div v-if="summary" class="mt-5 grid grid-cols-2 gap-4 lg:grid-cols-4">
      <StatCard :label="$t('common.status.pending')" :value="money(summary.pending, cur)" />
      <StatCard :label="$t('common.status.approved')" :value="money(summary.approved, cur)" :accent="tab === 'payable' && summary.approved > 0" :hint="tab === 'payable' ? $t('network.comm.readyToPay') : $t('network.comm.readyForPayout')" />
      <StatCard :label="$t('common.status.paid')" :value="money(summary.paid, cur)" />
      <StatCard :label="$t('common.status.void')" :value="money(summary.void, cur)" />
    </div>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>

    <div v-if="tab === 'earned'">
      <div v-if="earned?.items.length" class="card mt-5 overflow-x-auto">
        <table class="table">
          <thead><tr><th>{{ $t('common.order') }}</th><th>{{ $t('common.product') }}</th><th>{{ $t('network.supplier') }}</th><th>{{ $t('network.comm.amount') }}</th><th>{{ $t('network.status') }}</th></tr></thead>
          <tbody>
            <tr v-for="c in earned.items" :key="c.id">
              <td>#{{ shortId(c.order_id) }}<div class="text-xs text-muted">{{ ago(c.created_at) }}</div></td>
              <td>{{ c.product_name }}</td><td>{{ c.supplier_name }}</td>
              <td class="font-bold">{{ money(c.amount_cents, cur) }}</td>
              <td><StatusBadge :status="c.status" /></td>
            </tr>
          </tbody>
        </table>
      </div>
      <EmptyState v-else class="mt-5" :title="$t('network.comm.emptyTitle')" :text="$t('network.comm.emptyText')">
        <UButton to="/dashboard/marketplace">{{ $t('network.comm.findProducts') }}</UButton>
      </EmptyState>
    </div>
    <div v-else>
      <div v-if="payable?.items.length" class="card mt-5 overflow-x-auto">
        <table class="table">
          <thead><tr><th>{{ $t('common.order') }}</th><th>{{ $t('common.product') }}</th><th>{{ $t('network.sellStaff') }}</th><th>{{ $t('network.comm.amount') }}</th><th>{{ $t('network.status') }}</th><th /></tr></thead>
          <tbody>
            <tr v-for="c in payable.items" :key="c.id">
              <td>#{{ shortId(c.order_id) }}<div class="text-xs text-muted">{{ ago(c.created_at) }}</div></td>
              <td>{{ c.product_name }}</td><td>{{ c.reseller_name }}</td>
              <td class="font-bold">{{ money(c.amount_cents, cur) }}</td>
              <td><StatusBadge :status="c.status" /></td>
              <td class="text-right"><UButton size="sm" type="submit" v-if="c.status === 'approved'" @click="pay(c.id)">{{ $t('network.comm.markPaid') }}</UButton></td>
            </tr>
          </tbody>
        </table>
      </div>
      <EmptyState v-else class="mt-5" :title="$t('network.comm.nothingOwed')" />
    </div>
  </div>
</template>
