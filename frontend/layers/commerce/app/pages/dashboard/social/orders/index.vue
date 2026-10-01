<script setup lang="ts">
definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const route = useRoute()
const { t } = useI18n()
const { shopId, shop } = useShop()
const cur = computed(() => shop.value?.currency ?? 'THB')
interface Row { id: string; number: string; status: string; total_cents: number; customer_name: string; provider: string; has_address: boolean; expires_at: string; created_at: string; payment_ref: string; units: number; items: string }
const status = ref('')
const q = ref((route.query.q as string) || '')
const qd = ref(q.value)
let timer: ReturnType<typeof setTimeout> | undefined
watch(q, (v) => { clearTimeout(timer); timer = setTimeout(() => (qd.value = v), 250) })
const { data: rows } = await useAsyncData(
  'social-orders',
  () => api<Row[]>(`/shops/${shopId.value}/social/orders`, { query: { status: status.value || undefined, q: qd.value || undefined } }),
  { watch: [shopId, status, qd], default: () => [] },
)
const tabs = computed(() => [
  ['', t('common.all')],
  ['open', t('social.orders.tabs.open')],
  ['confirmed', t('social.orders.tabs.confirmed')],
  ['paid', t('social.orders.tabs.paid')],
  ['shipped', t('common.status.shipped')],
  ['completed', t('common.status.completed')],
  ['expired', t('common.status.expired')],
  ['cancelled', t('common.status.cancelled')],
] as const)
const hint = (r: Row) => {
  if (r.status === 'open') return t('social.orders.hintOpen', { date: fmtDateTime(r.expires_at, 'short', 'short') })
  if (r.status === 'confirmed') return r.payment_ref ? t('social.orders.hintPaidRef', { ref: r.payment_ref }) : t('social.orders.hintAddress')
  return ''
}
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('social.title') }}</h1>
    <SocialNav class="mt-5" />
    <div class="flex flex-wrap items-center gap-3">
      <div class="flex max-w-full gap-1 overflow-x-auto rounded-2xl bg-surface p-1 shadow-card text-sm font-semibold">
        <button v-for="tab in tabs" :key="tab[0]" class="whitespace-nowrap rounded-xl px-3 py-1" :class="status === tab[0] && 'bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]'" @click="status = tab[0]">{{ tab[1] }}</button>
      </div>
      <UInput v-model="q" class="w-56" :placeholder="$t('social.orders.searchPlaceholder')" />
    </div>
    <div v-if="rows.length" class="card mt-5 overflow-x-auto">
      <table class="table">
        <thead><tr><th>{{ $t('common.order') }}</th><th>{{ $t('common.customer') }}</th><th>{{ $t('social.orders.items') }}</th><th>{{ $t('common.total') }}</th><th>{{ $t('common.statusLabel') }}</th><th>{{ $t('common.created') }}</th></tr></thead>
        <tbody>
          <tr v-for="r in rows" :key="r.id" class="cursor-pointer hover:bg-paper" @click="navigateTo(`/dashboard/social/orders/${r.id}`)">
            <td class="font-mono font-bold">{{ r.number }}</td>
            <td><div class="flex items-center gap-2"><SocialProviderBadge :provider="r.provider" />{{ r.customer_name || '—' }}</div></td>
            <td class="max-w-64 truncate font-mono text-xs">{{ r.items }}</td>
            <td class="font-bold tabular-nums">{{ money(r.total_cents, cur) }}</td>
            <td>
              <StatusBadge :status="r.status" />
              <div v-if="hint(r)" class="mt-0.5 max-w-56 truncate text-[11px] text-muted">{{ hint(r) }}</div>
            </td>
            <td class="whitespace-nowrap text-muted">{{ ago(r.created_at) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <EmptyState v-else class="mt-5" :title="$t('social.orders.emptyTitle')" :text="$t('social.orders.emptyText')" >
      <UButton to="/dashboard/social">{{ $t('social.orders.openBoard') }}</UButton>
    </EmptyState>
  </div>
</template>
