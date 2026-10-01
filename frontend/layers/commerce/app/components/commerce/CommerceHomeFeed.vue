<script setup lang="ts">
/** Commerce section of the super-app home: ads + product feed with marketplace categories. */
import type { CatalogProduct } from '~/utils/types'

const api = useApi()
const route = useRoute()
const q = computed(() => (route.query.q as string) || '')
const category = computed(() => (route.query.category as string) || '')

const { data: products, status } = await useAsyncData(
  'catalog',
  () => api<CatalogProduct[]>('/catalog/products', { query: { q: q.value || undefined, category: category.value || undefined, limit: 48 } }),
  { watch: [q, category], default: () => [] },
)
const { data: categories } = await useMarketplaceCategories()
const current = computed(() => categories.value.find((c) => c.slug === category.value))
// Top-level category of the current selection, and its children for the second row.
const top = computed(() => {
  let c = current.value
  while (c?.parent_id) c = categories.value.find((x) => x.id === c!.parent_id)
  return c
})
const roots = computed(() => categories.value.filter((c) => !c.parent_id && c.total_count > 0))
const subs = computed(() => (top.value ? categories.value.filter((c) => c.parent_id === top.value!.id && c.total_count > 0) : []))
const trail = computed(() => current.value?.path.split(' › ') ?? [])
</script>

<template>
  <div id="shop">
    <section class="mx-auto max-w-7xl px-4 pt-10">
      <AdStrip :limit="3" />
    </section>

    <section class="mx-auto max-w-7xl px-4 pt-10">
      <div class="flex flex-wrap items-end justify-between gap-4">
        <h2 class="text-2xl font-extrabold tracking-tight">
          <template v-if="q">{{ $t('store.home.resultsFor', { q }) }}</template>
          <template v-else-if="current">{{ current.name }}</template>
          <template v-else>{{ $t('store.home.freshDrops') }}</template>
        </h2>
        <div class="flex flex-wrap gap-2">
          <NuxtLink to="/" class="rounded-xl px-3.5 py-1.5 text-sm font-semibold transition-all duration-200" :class="!category ? 'bg-brand-500 text-white shadow-[0_8px_18px_-8px_var(--brand-500)]' : 'bg-surface text-ink/75 shadow-card hover:-translate-y-0.5 hover:text-brand-500'">{{ $t('common.all') }}</NuxtLink>
          <NuxtLink
            v-for="c in roots"
            :key="c.id"
            :to="{ path: '/', query: { category: c.slug } }"
            class="rounded-xl px-3.5 py-1.5 text-sm font-semibold transition-all duration-200"
            :class="top?.id === c.id ? 'bg-brand-500 text-white shadow-[0_8px_18px_-8px_var(--brand-500)]' : 'bg-surface text-ink/75 shadow-card hover:-translate-y-0.5 hover:text-brand-500'"
          >{{ c.name }}</NuxtLink>
        </div>
      </div>
      <div v-if="subs.length" class="mt-3 flex flex-wrap items-center gap-2 text-sm">
        <span class="text-muted">{{ trail.slice(0, -1).join(' › ') || top?.name }}:</span>
        <NuxtLink :to="{ path: '/', query: { category: top!.slug } }" class="rounded-xl px-3 py-1 font-semibold" :class="current?.id === top?.id ? 'bg-brand-500/12 text-brand-600' : 'hover:bg-surface'">{{ $t('store.home.allIn', { name: top?.name }) }}</NuxtLink>
        <NuxtLink v-for="c in subs" :key="c.id" :to="{ path: '/', query: { category: c.slug } }" class="rounded-xl px-3 py-1 font-semibold" :class="current?.id === c.id || current?.parent_id === c.id ? 'bg-brand-500/12 text-brand-600' : 'hover:bg-surface'">
          {{ c.name }} <span class="font-normal text-muted">{{ c.total_count }}</span>
        </NuxtLink>
      </div>
      <div v-if="status === 'pending'" class="mt-6 grid grid-cols-2 gap-4 sm:grid-cols-3 sm:gap-5 lg:grid-cols-4">
        <div v-for="i in 8" :key="i" class="space-y-3"><div class="skeleton aspect-[4/5] rounded-[20px]" /><div class="skeleton h-4 w-3/4" /><div class="skeleton h-3 w-1/2" /></div>
      </div>
      <div v-else-if="products.length" class="mt-6 grid grid-cols-2 gap-4 sm:grid-cols-3 sm:gap-5 lg:grid-cols-4">
        <ProductCard v-for="p in products" :key="p.id" :product="p" />
      </div>
      <EmptyState v-else class="mt-6" :title="$t('store.home.emptyTitle')" :text="$t('store.home.emptyText')">
        <UButton to="/dashboard/products/new">{{ $t('store.home.listProduct') }}</UButton>
      </EmptyState>
    </section>
  </div>
</template>
