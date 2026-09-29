<script setup lang="ts">
import type { ProductCover } from '~/utils/types'
definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId, shop } = useShop()
interface MarketItem {
  id: string
  cover?: ProductCover | null
  shop_id: string
  name: string
  description: string
  price_cents: number
  images: string[]
  category: string
  stock: number
  commission_bps: number
  effective_commission_bps: number
  commission_per_unit_cents: number
  shop_name: string
  shop_slug: string
  partnership_status: string | null
  listed: boolean
}
interface Listing {
  id: string
  cover?: ProductCover | null
  product_id: string
  name: string
  price_cents: number
  images: string[]
  stock: number
  supplier_name: string
  commission_bps: number
  commission_per_unit_cents: number
  partnership_status: string | null
  live: boolean
}
const q = ref('')
const { data: items, refresh } = await useAsyncData('market', () => api<MarketItem[]>('/marketplace/products', { query: { shop_id: shopId.value, q: q.value || undefined } }), { watch: [shopId], default: () => [] })
const { data: listings, refresh: refreshListings } = await useAsyncData('listings', () => api<Listing[]>(`/shops/${shopId.value}/listings`), { watch: [shopId], default: () => [] })

const error = ref('')
const requesting = ref<MarketItem | null>(null)
const message = ref('')
async function request() {
  if (!requesting.value) return
  try {
    await api(`/shops/${shopId.value}/partnerships`, { method: 'POST', body: { supplier_shop_id: requesting.value.shop_id, message: message.value } })
    requesting.value = null
    message.value = ''
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
async function list(item: MarketItem) {
  error.value = ''
  try {
    await api(`/shops/${shopId.value}/listings`, { method: 'POST', body: { product_id: item.id } })
    await Promise.all([refresh(), refreshListings()])
  } catch (e) {
    error.value = apiError(e)
  }
}
async function unlist(l: Listing) {
  await api(`/listings/${l.id}`, { method: 'DELETE' })
  await Promise.all([refresh(), refreshListings()])
}
const copied = ref('')
function copyLink(productId: string) {
  const url = `${location.origin}/p/${productId}?via=${shopId.value}`
  navigator.clipboard?.writeText(url)
  copied.value = productId
  setTimeout(() => (copied.value = ''), 1500)
}
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('network.market.title') }}</h1>
    <i18n-t keypath="network.market.intro" tag="p" class="text-sm text-muted"><template #shop><b>{{ shop?.name }}</b></template></i18n-t>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>

    <section v-if="listings.length" class="mt-6">
      <h2 class="font-bold">{{ $t('network.market.inStorefrontN', { n: listings.length }) }}</h2>
      <div class="card mt-3 overflow-x-auto">
        <table class="table">
          <thead><tr><th>{{ $t('common.product') }}</th><th>{{ $t('network.supplier') }}</th><th>{{ $t('network.market.youEarn') }}</th><th>{{ $t('network.status') }}</th><th /></tr></thead>
          <tbody>
            <tr v-for="l in listings" :key="l.id">
              <td><div class="flex items-center gap-3"><ProductThumb :image="l.cover" :src="l.images?.[0]" :name="l.name" sizes="40px" class="size-10 shrink-0" rounded="rounded-lg" /><span class="font-semibold">{{ l.name }}</span></div></td>
              <td>{{ l.supplier_name }}</td>
              <td class="font-semibold">{{ money(l.commission_per_unit_cents, shop?.currency) }} <span class="text-xs text-muted">({{ pct(l.commission_bps) }})</span></td>
              <td><StatusBadge :status="l.live ? 'active' : 'paused'" /></td>
              <td class="whitespace-nowrap text-right">
                <UButton color="neutral" variant="soft" size="sm" type="submit" @click="copyLink(l.product_id)">{{ copied === l.product_id ? $t('common.copied') + ' ✓' : $t('network.market.copyLink') }}</UButton>
                <UButton color="neutral" variant="soft" size="sm" type="submit" class="ml-2" @click="unlist(l)">{{ $t('common.remove') }}</UButton>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <div class="mt-8 flex items-center gap-3">
      <h2 class="font-bold">{{ $t('common.nav.marketplace') }}</h2>
      <form class="ml-auto" @submit.prevent="refresh()"><UInput v-model="q" class="w-64" :placeholder="$t('network.market.search')" /></form>
    </div>
    <div v-if="items.length" class="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <div v-for="m in items" :key="m.id" class="card flex flex-col overflow-hidden">
        <ProductThumb :image="m.cover" :src="m.images?.[0]" :name="m.name" sizes="(min-width: 1024px) 25vw, 50vw" class="aspect-[4/3]" rounded="rounded-none" />
        <div class="flex flex-1 flex-col p-4">
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0"><div class="truncate font-bold">{{ m.name }}</div><div class="text-xs text-muted">{{ $t('network.market.byShop', { shop: m.shop_name }) }} · {{ $t('network.market.inStock', { n: num(m.stock) }) }}</div></div>
            <div class="font-bold">{{ money(m.price_cents, shop?.currency) }}</div>
          </div>
          <div class="mt-3 rounded-xl bg-brand-50 px-3 py-2 text-sm">
            <i18n-t keypath="network.market.earnPerUnit" tag="span"><template #amount><b>{{ money(m.commission_per_unit_cents, shop?.currency) }}</b></template></i18n-t> <span class="text-muted">({{ pct(m.effective_commission_bps) }})</span>
          </div>
          <div class="mt-auto pt-4">
            <UButton color="neutral" variant="soft" type="submit" v-if="m.listed" class="w-full" disabled>{{ $t('network.market.inStorefront') }} ✓</UButton>
            <UButton type="submit" v-else-if="m.partnership_status === 'approved'" class="w-full" @click="list(m)">{{ $t('network.market.addToStorefront') }}</UButton>
            <UButton color="neutral" variant="soft" type="submit" v-else-if="m.partnership_status === 'pending'" class="w-full" disabled>{{ $t('network.market.requestPending') }}</UButton>
            <UButton color="neutral" type="submit" v-else class="w-full" @click="requesting = m">{{ $t('network.market.requestToSell') }}</UButton>
          </div>
        </div>
      </div>
    </div>
    <EmptyState v-else class="mt-4" :title="$t('network.market.empty')" />

    <div v-if="requesting" class="fixed inset-0 z-50 grid place-items-center bg-night/40 p-4" @click.self="requesting = null">
      <form class="card w-full max-w-md space-y-4 p-6 shadow-2xl" @submit.prevent="request">
        <div class="text-lg font-bold">{{ $t('network.market.becomeFor', { shop: requesting.shop_name }) }}</div>
        <p class="text-sm text-muted">{{ $t('network.market.becomeHelp') }}</p>
        <UTextarea v-model="message" :rows="4" :placeholder="$t('network.market.messagePlaceholder')" />
        <div class="flex justify-end gap-2">
          <UButton color="neutral" variant="soft" type="button" @click="requesting = null">{{ $t('common.cancel') }}</UButton>
          <UButton type="submit">{{ $t('network.market.sendRequest') }}</UButton>
        </div>
      </form>
    </div>
  </div>
</template>
