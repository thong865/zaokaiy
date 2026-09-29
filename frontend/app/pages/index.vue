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
      <div class="relative overflow-hidden rounded-[28px] bg-night px-6 py-14 text-white shadow-[0_20px_50px_-20px_rgb(16_14_12/0.6)] sm:px-12 sm:py-20">
        <div class="absolute -right-24 -top-28 size-[28rem] rounded-full bg-brand-500/45 blur-3xl" />
        <div class="absolute -bottom-40 left-1/4 size-96 rounded-full bg-violet-500/25 blur-3xl" />
        <div class="absolute bottom-0 right-1/4 size-64 rounded-full bg-sun-400/15 blur-3xl" />
        <div class="absolute inset-0 bg-[radial-gradient(rgb(255_255_255/0.07)_1px,transparent_1px)] [background-size:22px_22px]" />
        <div class="relative max-w-2xl">
          <span class="chip gap-1.5 rounded-full bg-white/10 px-3 py-1 text-white backdrop-blur"><AppIcon name="sparkles" class="size-3.5 text-sun-400" />{{ $t('store.home.heroChip') }}</span>
          <h1 class="mt-5 text-4xl font-extrabold leading-[1.05] tracking-tight sm:text-6xl">
            {{ $t('store.home.heroTitle1') }}<br>{{ $t('store.home.heroTitle2') }} <span class="bg-gradient-to-r from-brand-500 to-sun-400 bg-clip-text text-transparent">{{ $t('store.home.heroTitle3') }}</span>
          </h1>
          <p class="mt-5 max-w-lg text-white/70">
            {{ $t('store.home.heroText') }}
          </p>
          <div class="mt-8 flex flex-wrap gap-3">
            <UButton size="xl" :to="loggedIn ? '/dashboard' : '/register'">{{ $t('store.home.startSelling') }} <AppIcon name="chevron-right" class="size-4" /></UButton>
            <UButton color="neutral" variant="ghost" size="xl" :to="loggedIn ? '/dashboard/marketplace' : '/register'" class="bg-white/10 text-white backdrop-blur hover:bg-white/20">{{ $t('store.home.becomeSellStaff') }}</UButton>
          </div>
        </div>
      </div>
      <div class="mt-6 grid gap-4 sm:grid-cols-3">
        <div v-for="(f, i) in ['ownShop', 'network', 'ai']" :key="f" class="card card-hover flex gap-4 p-5">
          <span class="icon-tile" :class="['bg-brand-500/12 text-brand-600', 'bg-mint-500/12 text-mint-500', 'bg-violet-500/12 text-violet-500'][i]"><AppIcon :name="['store', 'users', 'sparkles'][i]!" class="size-5" /></span>
          <div>
            <div class="font-bold">{{ $t(`store.home.features.${f}.title`) }}</div>
            <div class="mt-1 text-sm text-muted">{{ $t(`store.home.features.${f}.text`) }}</div>
          </div>
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
