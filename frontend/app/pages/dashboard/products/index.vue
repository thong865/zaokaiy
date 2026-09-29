<script setup lang="ts">
import type { Product } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId, shop } = useShop()
const q = ref('')
const status = ref('')
const review = ref('')
const shopCat = ref('')
const { data: shopCats } = await useShopCategories()
const catName = (id: string | null) => shopCats.value.find((c) => c.id === id)?.name
const { data: products } = await useAsyncData(
  'products',
  () =>
    api<Product[]>(`/shops/${shopId.value}/products`, {
      query: { q: q.value || undefined, status: status.value || undefined, review_status: review.value || undefined, shop_category_id: shopCat.value || undefined },
    }),
  { watch: [shopId, status, review, shopCat], default: () => [] },
)
const debounced = refDebounced(q)
watch(debounced, () => refreshNuxtData('products'))

function refDebounced(src: Ref<string>, ms = 300) {
  const out = ref(src.value)
  let t: ReturnType<typeof setTimeout> | undefined
  watch(src, (v) => { clearTimeout(t); t = setTimeout(() => (out.value = v), ms) })
  return out
}
</script>

<template>
  <div>
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h1 class="page-title">{{ $t('product.list.title') }}</h1>
      <UButton to="/dashboard/products/new">{{ $t('product.list.newProduct') }}</UButton>
    </div>
    <div class="mt-5 flex flex-wrap gap-3">
      <UInput v-model="q" :placeholder="$t('product.list.searchPh')" class="max-w-xs" />
      <select v-model="status" class="input w-40">
        <option value="">{{ $t('product.list.allStatuses') }}</option><option value="active">{{ $t('common.status.active') }}</option><option value="draft">{{ $t('common.status.draft') }}</option><option value="archived">{{ $t('common.status.archived') }}</option>
      </select>
      <select v-model="review" class="input w-44">
        <option value="">{{ $t('product.list.anyReview') }}</option>
        <option v-for="k in ['not_submitted', 'pending', 'approved', 'rejected']" :key="k" :value="k">{{ reviewLabel(k) }}</option>
      </select>
      <select v-if="shopCats.length" v-model="shopCat" class="input w-48">
        <option value="">{{ $t('product.list.allShopCats') }}</option>
        <option v-for="c in shopCats" :key="c.id" :value="c.id">{{ '— '.repeat(c.depth) }}{{ c.name }}</option>
      </select>
    </div>
    <div v-if="products.length" class="card mt-5 overflow-x-auto">
      <table class="table">
        <thead><tr><th>{{ $t('common.product') }}</th><th>{{ $t('common.price') }}</th><th>{{ $t('common.stock') }}</th><th>{{ $t('product.list.sellStaff') }}</th><th>{{ $t('common.statusLabel') }}</th><th>{{ $t('product.list.review') }}</th></tr></thead>
        <tbody>
          <tr v-for="p in products" :key="p.id" class="cursor-pointer hover:bg-paper" @click="navigateTo(`/dashboard/products/${p.id}`)">
            <td>
              <div class="flex items-center gap-3">
                <ProductThumb :image="p.cover" :src="p.images?.[0]" :name="p.name" sizes="40px" class="size-10 shrink-0" rounded="rounded-lg" />
                <div><div class="font-semibold">{{ p.name }}</div><div class="text-xs text-muted">{{ p.sku }}<template v-if="catName(p.shop_category_id)"> · {{ catName(p.shop_category_id) }}</template></div></div>
              </div>
            </td>
            <td class="font-semibold">{{ money(p.price_cents, shop?.currency) }}</td>
            <td><span :class="p.stock <= p.low_stock_threshold && 'font-bold text-brand-700'">{{ p.stock }}</span></td>
            <td><span v-if="p.allow_resell" class="chip bg-brand-50 text-brand-700">{{ pct(p.commission_bps) }}</span><span v-else class="text-muted">—</span></td>
            <td><StatusBadge :status="p.status" /></td>
            <td>
              <StatusBadge :status="p.review_status" :label="reviewLabel(p.review_status)" />
              <div v-if="p.review_status === 'rejected' && p.review_note" class="mt-1 max-w-48 truncate text-[11px] text-brand-700" :title="p.review_note">{{ p.review_note }}</div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <EmptyState v-else class="mt-5" :title="$t('product.list.emptyTitle')" :text="$t('product.list.emptyText')">
      <UButton to="/dashboard/products/new">{{ $t('product.list.add') }}</UButton>
    </EmptyState>
  </div>
</template>
