<script setup lang="ts">
import type { CatalogProduct, CategoryNode, Content, Shop } from '~/utils/types'

const route = useRoute()
const api = useApi()
const { data, error } = await useAsyncData(`store:${route.params.slug}`, () =>
  api<{ shop: Shop; products: CatalogProduct[]; resold: CatalogProduct[]; contents: Content[]; categories: CategoryNode[] }>(`/storefront/${route.params.slug}`),
)
if (error.value || !data.value) throw createError({ statusCode: 404, statusMessage: tr('store.shop.notFound'), fatal: true })
const tab = ref<'all' | 'own' | 'picks'>('all')
const cat = ref<string | null>(null)
const cats = computed(() => (data.value?.categories ?? []).filter((c) => c.total_count > 0))
const list = computed(() => {
  const d = data.value!
  if (cat.value) {
    const ids = subtreeIds(d.categories, cat.value)
    return d.products.filter((p) => p.shop_category_id && ids.has(p.shop_category_id))
  }
  if (tab.value === 'own') return d.products
  if (tab.value === 'picks') return d.resold
  return [...d.products, ...d.resold]
})
function pickCat(id: string | null) {
  cat.value = id
  tab.value = 'all'
}
useHead({ title: () => `${data.value?.shop.name} · zaokaiy` })
</script>

<template>
  <div v-if="data" class="mx-auto max-w-7xl px-4 pt-10">
    <div class="card flex flex-wrap items-center gap-5 p-6">
      <ProductThumb :src="data.shop.logo_url" :name="data.shop.name" class="size-20" rounded="rounded-3xl" />
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2">
          <h1 class="text-2xl font-extrabold tracking-tight">{{ data.shop.name }}</h1>
          <span class="chip bg-brand-50 capitalize text-brand-700">{{ $te(`store.shopKind.${data.shop.kind}`) ? $t(`store.shopKind.${data.shop.kind}`) : data.shop.kind }}</span>
        </div>
        <p class="mt-1 max-w-2xl text-sm text-muted">{{ data.shop.description || $t('store.shop.welcome') }}</p>
      </div>
      <div class="flex gap-6 text-center text-sm">
        <div><div class="text-xl font-extrabold">{{ data.products.length }}</div><div class="text-muted">{{ $t('store.shop.ownProducts') }}</div></div>
        <div><div class="text-xl font-extrabold">{{ data.resold.length }}</div><div class="text-muted">{{ $t('store.shop.curatedPicks') }}</div></div>
      </div>
    </div>

    <div class="mt-8 grid gap-8" :class="cats.length ? 'lg:grid-cols-[220px_1fr]' : ''">
      <aside v-if="cats.length" class="lg:sticky lg:top-24 lg:self-start">
        <div class="mb-2 text-xs font-bold uppercase tracking-wider text-muted">{{ $t('store.shop.categories') }}</div>
        <nav class="flex gap-1 overflow-x-auto pb-1 lg:flex-col lg:overflow-visible">
          <button class="shrink-0 rounded-xl px-3 py-2 text-left text-sm font-semibold" :class="!cat ? 'bg-ink text-white' : 'hover:bg-white'" @click="pickCat(null)">{{ $t('store.shop.allProducts') }}</button>
          <button
            v-for="c in cats"
            :key="c.id"
            class="flex shrink-0 items-center justify-between gap-2 rounded-xl px-3 py-2 text-left text-sm"
            :class="[cat === c.id ? 'bg-ink text-white' : 'hover:bg-white', c.depth ? 'font-medium' : 'font-semibold']"
            :style="{ paddingLeft: `${12 + c.depth * 14}px` }"
            @click="pickCat(c.id)"
          >
            <span class="truncate">{{ c.name }}</span><span class="text-xs opacity-60">{{ c.total_count }}</span>
          </button>
        </nav>
      </aside>
      <div>
        <div v-if="!cat" class="flex gap-2">
          <button v-for="t in (['all', 'own', 'picks'] as const)" :key="t" class="chip border px-3 py-1.5 capitalize" :class="tab === t ? 'border-ink bg-ink text-white' : 'border-line bg-white'" @click="tab = t">
            {{ t === 'picks' ? $t('store.shop.curatedPicks') : t === 'own' ? $t('store.shop.ownProducts') : $t('common.all') }}
          </button>
        </div>
        <div v-else class="text-sm text-muted">{{ cats.find((c) => c.id === cat)?.path }}</div>
        <div v-if="list.length" class="mt-6 grid grid-cols-2 gap-x-6 gap-y-10 sm:grid-cols-3" :class="cats.length ? '' : 'lg:grid-cols-4'">
          <ProductCard v-for="p in list" :key="`${p.id}-${p.via_shop_id}`" :product="p" />
        </div>
        <EmptyState v-else class="mt-6" :title="$t('store.shop.noProducts')" />
      </div>
    </div>

    <section v-if="data.contents.length" class="mt-14">
      <h2 class="text-xl font-extrabold tracking-tight">{{ $t('store.shop.fromShop', { name: data.shop.name }) }}</h2>
      <div class="mt-4 grid gap-4 md:grid-cols-3">
        <article v-for="c in data.contents" :key="c.id" class="card p-5">
          <div class="text-[11px] font-bold uppercase tracking-wider text-muted">{{ kindLabel(c.kind) }}</div>
          <h3 class="mt-1 font-bold">{{ c.title }}</h3>
          <p class="mt-2 line-clamp-6 whitespace-pre-line text-sm text-ink/80">{{ c.body }}</p>
        </article>
      </div>
    </section>
  </div>
</template>
