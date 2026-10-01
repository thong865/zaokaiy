<script setup lang="ts">
/** Dashboard overview for an insurance agent: applications by stage and premiums. */
import type { Application, Plan } from '#insurance/utils/insurance'

const api = useApi()
const { shop, shopId } = useShop()
const { data } = await useAsyncData('ins-overview', async () => {
  const [apps, plans] = await Promise.all([
    api<Application[]>(`/shops/${shopId.value}/insurance/applications`),
    api<Plan[]>(`/shops/${shopId.value}/insurance/plans`),
  ])
  return { apps, plans }
}, { watch: [shopId] })
const cur = computed(() => shop.value?.currency ?? 'LAK')
const count = (s: string[]) => data.value?.apps.filter((a) => s.includes(a.status)).length ?? 0
const premiums = computed(() => data.value?.apps.filter((a) => a.status === 'issued' && a.paid).reduce((n, a) => n + a.premium_cents, 0) ?? 0)
</script>

<template>
  <div v-if="data">
    <div class="flex flex-wrap items-end justify-between gap-3">
      <h1 class="page-title">{{ $t('dash.overview.hello', { name: shop?.name }) }}</h1>
      <div class="flex gap-2">
        <UButton color="neutral" variant="soft" to="/dashboard/insurance/plans">{{ $t('insurance.agent.plansTitle') }}</UButton>
        <UButton to="/dashboard/insurance/applications">{{ $t('insurance.agent.appsTitle') }}</UButton>
      </div>
    </div>
    <div class="mt-6 grid grid-cols-2 gap-4 lg:grid-cols-4">
      <StatCard :label="$t('insurance.agent.stat.new')" :value="count(['submitted', 'reviewing'])" :hint="$t('insurance.agent.stat.newHint')" to="/dashboard/insurance/applications" accent />
      <StatCard :label="$t('insurance.agent.stat.toIssue')" :value="count(['approved'])" :hint="$t('insurance.agent.stat.toIssueHint')" to="/dashboard/insurance/applications" />
      <StatCard :label="$t('insurance.agent.stat.issued')" :value="count(['issued'])" :hint="$t('insurance.agent.stat.issuedHint', { amount: money(premiums, cur) })" />
      <StatCard :label="$t('insurance.agent.stat.plans')" :value="data.plans.filter((p) => p.active).length" :hint="$t('insurance.agent.stat.plansHint')" to="/dashboard/insurance/plans" />
    </div>
  </div>
</template>
