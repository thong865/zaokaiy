<script setup lang="ts">
/** Dashboard overview for a restaurant: today's orders and takings, quick links. */
const { data } = useRestaurant()
const { shop } = useShop()
const cur = computed(() => data.value?.shop.currency ?? shop.value?.currency ?? 'LAK')
</script>

<template>
  <div v-if="data">
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 class="page-title">{{ $t('dash.overview.hello', { name: shop?.name }) }}</h1>
        <p class="text-sm text-muted">{{ data.settings.accepting_orders ? $t('restaurant.overview.acceptingNow') : $t('restaurant.overview.notAccepting') }}</p>
      </div>
      <div class="flex gap-2">
        <UButton color="neutral" variant="soft" :to="`/s/${data.shop.slug}`" target="_blank" icon="i-lucide-external-link">{{ $t('restaurant.board.openMenu') }}</UButton>
        <UButton to="/dashboard/restaurant/orders" icon="i-lucide-chef-hat">{{ $t('restaurant.board.title') }}</UButton>
      </div>
    </div>
    <div class="mt-6 grid grid-cols-2 gap-4 lg:grid-cols-4">
      <StatCard :label="$t('restaurant.overview.revenue')" :value="money(data.today.revenue_cents, cur)" :hint="$t('restaurant.overview.revenueHint')" accent />
      <StatCard :label="$t('restaurant.overview.orders')" :value="data.today.orders" :hint="$t('restaurant.overview.ordersHint')" to="/dashboard/restaurant/orders" />
      <StatCard :label="$t('restaurant.overview.open')" :value="data.today.open" :hint="$t('restaurant.overview.openHint')" to="/dashboard/restaurant/orders" />
      <StatCard :label="$t('restaurant.overview.dishes')" :value="data.items.filter((i) => i.available).length" :hint="$t('restaurant.overview.dishesHint', { n: data.tables.length })" to="/dashboard/restaurant/menu" />
    </div>
    <div v-if="!data.items.length || !data.tables.length" class="card mt-4 p-5">
      <div class="font-bold">{{ $t('restaurant.overview.setupTitle') }}</div>
      <ol class="mt-3 space-y-2 text-sm">
        <li :class="data.items.length > 0 && 'text-muted line-through'">1. <NuxtLink to="/dashboard/restaurant/menu" class="font-semibold text-brand-600">{{ $t('restaurant.overview.setupMenu') }}</NuxtLink></li>
        <li :class="data.tables.length > 0 && 'text-muted line-through'">2. <NuxtLink to="/dashboard/restaurant/tables" class="font-semibold text-brand-600">{{ $t('restaurant.overview.setupTables') }}</NuxtLink></li>
        <li>3. <NuxtLink to="/dashboard/restaurant/settings" class="font-semibold text-brand-600">{{ $t('restaurant.overview.setupSettings') }}</NuxtLink></li>
      </ol>
    </div>
  </div>
</template>
