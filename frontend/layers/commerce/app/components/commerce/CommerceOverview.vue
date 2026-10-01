<script setup lang="ts">
/** Commerce dashboard overview: sales, orders to ship, stock, sell-staff, commissions, ads. */
const api = useApi()
const { shop, shopId, can } = useShop()

interface Stats {
  revenue_cents: number
  orders: number
  units_sold: number
  orders_to_ship: number
  products: number
  active_products: number
  in_review: number
  rejected: number
  low_stock: number
  commission_earned_cents: number
  commission_owed_cents: number
  ad_spend_cents: number
  ad_clicks: number
  ad_impressions: number
  resellers: number
  pending_reseller_requests: number
  revenue_series: { date: string; revenue_cents: number }[]
  top_products: { name: string; units: number; revenue_cents: number }[]
}
const { data: stats } = await useAsyncData('stats', () => (shopId.value && can('stats') ? api<Stats>(`/shops/${shopId.value}/stats`) : Promise.resolve(null)), {
  watch: [shopId],
})

const cur = computed(() => shop.value?.currency ?? 'THB')
const ctr = computed(() => (stats.value?.ad_impressions ? ((stats.value.ad_clicks / stats.value.ad_impressions) * 100).toFixed(1) + '%' : '—'))
</script>

<template>
  <div>
  <div v-if="stats">
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 class="page-title">{{ $t('dash.overview.hello', { name: shop?.name }) }}</h1>
        <p class="text-sm text-muted">{{ $t('dash.overview.subtitle') }}</p>
      </div>
      <div class="flex gap-2">
        <UButton color="neutral" variant="soft" to="/dashboard/assistant">{{ $t('dash.overview.askAi') }}</UButton>
        <UButton to="/dashboard/products/new">{{ $t('dash.overview.newProduct') }}</UButton>
      </div>
    </div>

    <NuxtLink v-if="stats.rejected || stats.in_review" to="/dashboard/products" class="mt-6 flex flex-wrap items-center gap-3 rounded-2xl border px-4 py-3 text-sm" :class="stats.rejected ? 'border-brand-500/40 bg-brand-50' : 'border-sun-400/60 bg-sun-400/10'">
      <i18n-t v-if="stats.rejected" keypath="dash.overview.needChanges" :plural="stats.rejected" tag="span"><template #n><b>{{ stats.rejected }}</b></template></i18n-t>
      <i18n-t v-if="stats.in_review" keypath="dash.overview.waitingReview" tag="span"><template #n><b>{{ stats.in_review }}</b></template></i18n-t>
      <span class="ml-auto font-semibold underline">{{ $t('dash.overview.viewProducts') }}</span>
    </NuxtLink>
    <div class="mt-6 grid grid-cols-2 gap-4 lg:grid-cols-4">
      <StatCard :label="$t('dash.overview.revenue')" :value="money(stats.revenue_cents, cur)" :hint="$t('dash.overview.revenueHint', { units: num(stats.units_sold), orders: num(stats.orders) })" accent />
      <StatCard :label="$t('dash.overview.toShip')" :value="stats.orders_to_ship" :hint="$t('dash.overview.toShipHint')" to="/dashboard/orders" />
      <StatCard :label="$t('dash.overview.lowStock')" :value="stats.low_stock" :hint="$t('dash.overview.lowStockHint', { live: stats.active_products, total: stats.products })" to="/dashboard/inventory" />
      <StatCard :label="$t('dash.overview.sellStaff')" :value="stats.resellers" :hint="$t('dash.overview.sellStaffHint', { n: stats.pending_reseller_requests })" to="/dashboard/partners" />
    </div>

    <div class="mt-4 grid gap-4 lg:grid-cols-[2fr_1fr]">
      <div class="card p-5">
        <div class="font-bold">{{ $t('dash.overview.revenue') }}</div>
        <RevenueChart class="mt-3" :series="stats.revenue_series" :currency="cur" />
      </div>
      <div class="card p-5">
        <div class="font-bold">{{ $t('dash.overview.topProducts') }}</div>
        <ol v-if="stats.top_products.length" class="mt-3 space-y-3 text-sm">
          <li v-for="(t, i) in stats.top_products" :key="t.name" class="flex items-center gap-3">
            <span class="grid size-6 place-items-center rounded-full bg-paper text-xs font-bold">{{ i + 1 }}</span>
            <span class="flex-1 truncate">{{ t.name }}</span>
            <span class="font-semibold">{{ money(t.revenue_cents, cur) }}</span>
          </li>
        </ol>
        <p v-else class="mt-3 text-sm text-muted">{{ $t('dash.overview.noSales') }}</p>
      </div>
    </div>

    <div class="mt-4 grid grid-cols-2 gap-4 lg:grid-cols-4">
      <StatCard :label="$t('dash.overview.commissionEarned')" :value="money(stats.commission_earned_cents, cur)" :hint="$t('dash.overview.commissionEarnedHint')" to="/dashboard/commissions" />
      <StatCard :label="$t('dash.overview.commissionOwed')" :value="money(stats.commission_owed_cents, cur)" :hint="$t('dash.overview.commissionOwedHint')" to="/dashboard/commissions" />
      <StatCard :label="$t('dash.overview.adSpend')" :value="money(stats.ad_spend_cents, cur)" :hint="$t('dash.overview.adSpendHint', { n: num(stats.ad_clicks) })" to="/dashboard/ads" />
      <StatCard :label="$t('dash.overview.adCtr')" :value="ctr" :hint="$t('dash.overview.adCtrHint', { n: num(stats.ad_impressions) })" to="/dashboard/ads" />
    </div>
  </div>
  </div>
</template>
