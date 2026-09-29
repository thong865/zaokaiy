<script setup lang="ts">
const props = withDefaults(defineProps<{ limit?: number }>(), { limit: 3 })
const api = useApi()
interface ServedAd {
  id: string
  headline: string
  body: string
  image_url: string | null
  product_name: string
  price_cents: number
  shop_name: string
  shop_slug: string
}
// Served client-side so impressions are counted per real view, not per SSR render.
const { data: ads } = useLazyAsyncData('ads', () => api<ServedAd[]>('/ads/serve', { query: { limit: props.limit } }), {
  server: false,
  default: () => [],
})

async function open(ad: ServedAd) {
  try {
    const r = await api<{ product_id: string; via_shop_id: string | null }>(`/ads/${ad.id}/click`, { method: 'POST' })
    await navigateTo({ path: `/p/${r.product_id}`, query: r.via_shop_id ? { via: r.via_shop_id } : {} })
  } catch {
    /* campaign exhausted between serve and click */
  }
}
</script>

<template>
  <div v-if="ads?.length" class="grid gap-3 sm:grid-cols-3">
    <button
      v-for="ad in ads"
      :key="ad.id"
      class="card group flex items-center gap-3 p-3 text-left transition hover:border-ink/40"
      @click="open(ad)"
    >
      <ProductThumb :src="ad.image_url" :name="ad.product_name" class="size-16 shrink-0" rounded="rounded-xl" />
      <div class="min-w-0">
        <div class="flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-wider text-muted">
          <span class="rounded bg-ink px-1 text-paper">{{ $t('store.ad') }}</span> {{ ad.shop_name }}
        </div>
        <div class="line-clamp-1 font-semibold group-hover:underline">{{ ad.headline }}</div>
        <div class="line-clamp-1 text-xs text-muted">{{ ad.body || ad.product_name }}</div>
      </div>
      <div class="ml-auto shrink-0 text-sm font-bold">{{ money(ad.price_cents) }}</div>
    </button>
  </div>
</template>
