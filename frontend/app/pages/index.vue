<script setup lang="ts">
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
const { loggedIn } = useAuth()
</script>

<template>
  <div>
    <section v-if="!q && !category" class="mx-auto max-w-7xl px-4 pt-10">
      <div class="relative overflow-hidden rounded-[2rem] bg-ink px-6 py-14 text-white sm:px-12 sm:py-20">
        <div class="absolute -right-24 -top-24 size-96 rounded-full bg-brand-500/40 blur-3xl" />
        <div class="absolute -bottom-32 left-1/3 size-80 rounded-full bg-sun-400/20 blur-3xl" />
        <div class="relative max-w-2xl">
          <span class="chip bg-white/10 text-white">{{ $t('store.home.heroChip') }}</span>
          <h1 class="mt-5 text-4xl font-extrabold leading-[1.05] tracking-tight sm:text-6xl">
            {{ $t('store.home.heroTitle1') }}<br>{{ $t('store.home.heroTitle2') }} <span class="text-brand-500">{{ $t('store.home.heroTitle3') }}</span>
          </h1>
          <p class="mt-5 max-w-lg text-white/70">
            {{ $t('store.home.heroText') }}
          </p>
          <div class="mt-8 flex flex-wrap gap-3">
            <NuxtLink :to="loggedIn ? '/dashboard' : '/register'" class="btn-primary">{{ $t('store.home.startSelling') }}</NuxtLink>
            <NuxtLink :to="loggedIn ? '/dashboard/marketplace' : '/register'" class="btn bg-white/10 text-white hover:bg-white/20">{{ $t('store.home.becomeSellStaff') }}</NuxtLink>
          </div>
        </div>
      </div>
      <div class="mt-6 grid gap-3 sm:grid-cols-3">
        <div v-for="f in ['ownShop', 'network', 'ai']" :key="f" class="card p-5">
          <div class="font-bold">{{ $t(`store.home.features.${f}.title`) }}</div>
          <div class="mt-1 text-sm text-muted">{{ $t(`store.home.features.${f}.text`) }}</div>
        </div>
      </div>
    </section>

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
          <NuxtLink to="/" class="chip border px-3 py-1" :class="!category ? 'border-ink bg-ink text-white' : 'border-line bg-white'">{{ $t('common.all') }}</NuxtLink>
          <NuxtLink
            v-for="c in roots"
            :key="c.id"
            :to="{ path: '/', query: { category: c.slug } }"
            class="chip border px-3 py-1"
            :class="top?.id === c.id ? 'border-ink bg-ink text-white' : 'border-line bg-white'"
          >{{ c.name }}</NuxtLink>
        </div>
      </div>
      <div v-if="subs.length" class="mt-3 flex flex-wrap items-center gap-2 text-sm">
        <span class="text-muted">{{ trail.slice(0, -1).join(' › ') || top?.name }}:</span>
        <NuxtLink :to="{ path: '/', query: { category: top!.slug } }" class="rounded-full px-3 py-1 font-semibold" :class="current?.id === top?.id ? 'bg-brand-50 text-brand-700' : 'hover:bg-white'">{{ $t('store.home.allIn', { name: top?.name }) }}</NuxtLink>
        <NuxtLink v-for="c in subs" :key="c.id" :to="{ path: '/', query: { category: c.slug } }" class="rounded-full px-3 py-1 font-semibold" :class="current?.id === c.id || current?.parent_id === c.id ? 'bg-brand-50 text-brand-700' : 'hover:bg-white'">
          {{ c.name }} <span class="font-normal text-muted">{{ c.total_count }}</span>
        </NuxtLink>
      </div>
      <div v-if="status === 'pending'" class="mt-6 grid grid-cols-2 gap-6 sm:grid-cols-3 lg:grid-cols-4">
        <div v-for="i in 8" :key="i" class="aspect-[4/5] animate-pulse rounded-2xl bg-line/60" />
      </div>
      <div v-else-if="products.length" class="mt-6 grid grid-cols-2 gap-x-6 gap-y-10 sm:grid-cols-3 lg:grid-cols-4">
        <ProductCard v-for="p in products" :key="p.id" :product="p" />
      </div>
      <EmptyState v-else class="mt-6" :title="$t('store.home.emptyTitle')" :text="$t('store.home.emptyText')">
        <NuxtLink to="/dashboard/products/new" class="btn-primary">{{ $t('store.home.listProduct') }}</NuxtLink>
      </EmptyState>
    </section>
  </div>
</template>
