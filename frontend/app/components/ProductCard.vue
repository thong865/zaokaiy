<script setup lang="ts">
import type { CatalogProduct } from '~/utils/types'

const props = defineProps<{ product: CatalogProduct }>()
const to = computed(() => ({
  path: `/p/${props.product.id}`,
  query: props.product.via_shop_id ? { via: props.product.via_shop_id } : {},
}))
const { add } = useCart()
const added = ref(false)
function quickAdd() {
  const p = props.product
  add({
    product_id: p.id,
    via_shop_id: p.via_shop_id,
    name: p.name,
    price_cents: p.price_cents,
    currency: p.currency,
    image: p.images?.[0] ?? null,
    shop_name: p.shop_name,
  })
  added.value = true
  setTimeout(() => (added.value = false), 1200)
}
</script>

<template>
  <div class="group">
    <NuxtLink :to="to" class="block">
      <ProductThumb :src="product.images?.[0]" :name="product.name" class="aspect-[4/5] transition group-hover:-translate-y-0.5" />
    </NuxtLink>
    <div class="mt-3 flex items-start justify-between gap-2">
      <div class="min-w-0">
        <NuxtLink :to="to" class="line-clamp-1 font-semibold hover:underline">{{ product.name }}</NuxtLink>
        <NuxtLink :to="`/s/${product.shop_slug}`" class="text-xs text-muted hover:text-ink">{{ product.shop_name }}</NuxtLink>
      </div>
      <div class="shrink-0 text-right font-bold">{{ money(product.price_cents, product.currency) }}</div>
    </div>
    <div class="mt-2 flex items-center justify-between">
      <span v-if="product.stock <= 0" class="chip bg-line text-muted">{{ $t('store.card.soldOut') }}</span>
      <span v-else-if="product.stock <= 5" class="chip bg-sun-400/30 text-ink">{{ $t('store.card.onlyLeft', { n: product.stock }) }}</span>
      <span v-else />
      <button class="btn-ghost btn-sm" :disabled="product.stock <= 0" @click="quickAdd">
        {{ added ? $t('store.card.added') : $t('store.card.addToCart') }}
      </button>
    </div>
  </div>
</template>
