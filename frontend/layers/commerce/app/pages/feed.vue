<script setup lang="ts">
const api = useApi()
interface FeedItem {
  id: string
  kind: string
  title: string
  body: string
  media_url: string | null
  created_at: string
  shop: { id: string; name: string; slug: string }
  product: { id: string; name: string; price_cents: number; via_shop_id: string | null } | null
}
const { data: feed } = await useAsyncData('feed', () => api<FeedItem[]>('/feed/contents', { query: { limit: 50 } }), { default: () => [] })
const { t } = useI18n()
useHead({ title: () => t('common.nav.creators') + ' · zaokaiy' })
</script>

<template>
  <div class="mx-auto max-w-3xl px-4 pt-10">
    <h1 class="page-title text-3xl">{{ $t('store.feed.heading') }}</h1>
    <p class="mt-1 text-muted">{{ $t('store.feed.intro') }}</p>
    <div v-if="feed.length" class="mt-8 space-y-5">
      <article v-for="f in feed" :key="f.id" class="card overflow-hidden">
        <video v-if="f.media_url && isVideoUrl(f.media_url)" :src="f.media_url" controls playsinline preload="metadata" class="aspect-video w-full bg-night object-cover" />
        <img v-else-if="f.media_url" :src="f.media_url" class="aspect-video w-full object-cover" alt="" >
        <div class="p-5">
          <div class="flex items-center gap-2 text-sm">
            <span class="grid size-8 place-items-center rounded-full bg-ink text-xs font-bold text-paper">{{ f.shop.name.slice(0, 1) }}</span>
            <NuxtLink :to="`/s/${f.shop.slug}`" class="font-bold hover:underline">{{ f.shop.name }}</NuxtLink>
            <span class="text-muted">· {{ ago(f.created_at) }} · {{ kindLabel(f.kind) }}</span>
          </div>
          <h2 class="mt-3 text-lg font-bold">{{ f.title }}</h2>
          <p class="mt-2 whitespace-pre-line text-sm text-ink/80">{{ f.body }}</p>
          <NuxtLink
            v-if="f.product"
            :to="{ path: `/p/${f.product.id}`, query: f.product.via_shop_id ? { via: f.product.via_shop_id } : {} }"
            class="mt-4 flex items-center justify-between rounded-2xl bg-paper px-4 py-3 text-sm hover:bg-brand-50"
          >
            <span class="font-semibold">🛒 {{ f.product.name }}</span>
            <span class="font-bold">{{ money(f.product.price_cents) }} →</span>
          </NuxtLink>
        </div>
      </article>
    </div>
    <EmptyState v-else class="mt-8" :title="$t('store.feed.emptyTitle')" :text="$t('store.feed.emptyText')" />
  </div>
</template>
