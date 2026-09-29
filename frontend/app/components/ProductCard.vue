<script setup lang="ts">
import type { CatalogProduct } from '~/utils/types'

const props = defineProps<{ product: CatalogProduct }>()
const to = computed(() => ({
  path: `/p/${props.product.id}`,
  query: props.product.via_shop_id ? { via: props.product.via_shop_id } : {},
}))
const veh = computed(() => props.product.vehicle ?? null)
const cartable = computed(() => !veh.value || (veh.value.buy_online && veh.value.sale_status === 'available'))
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
    shop_id: p.shop_id,
    supplier_name: p.shop_name,
  })
  added.value = true
  setTimeout(() => (added.value = false), 1200)
}
</script>

<template>
  <div class="group card card-hover flex flex-col overflow-hidden">
    <NuxtLink :to="to" class="relative block overflow-hidden">
      <ProductThumb :image="product.cover" :src="product.images?.[0]" :name="product.name" rounded="rounded-none" sizes="(min-width: 1280px) 20vw, (min-width: 768px) 30vw, 50vw" class="aspect-[4/5] transition-transform duration-500 group-hover:scale-[1.04]" />
      <span v-if="veh && veh.sale_status !== 'available'" class="chip absolute left-3 top-3 font-bold text-white" :class="veh.sale_status === 'sold' ? 'bg-night' : 'bg-amber-600'">{{ optLabel('sale', veh.sale_status) }}</span>
      <span v-else-if="!veh && product.stock <= 0" class="chip absolute left-3 top-3 bg-night/80 text-white backdrop-blur">{{ $t('store.card.soldOut') }}</span>
      <span v-else-if="!veh && product.stock <= 5" class="chip absolute left-3 top-3 bg-sun-400 text-night">{{ $t('store.card.onlyLeft', { n: product.stock }) }}</span>
      <span v-if="product.shop_verified" class="absolute right-3 top-3 grid size-7 place-items-center rounded-full bg-surface/90 text-mint-500 shadow backdrop-blur" :title="$t('kyb.badge.verified')"><AppIcon name="check-circle" class="size-4" /></span>
    </NuxtLink>
    <div class="flex flex-1 flex-col p-3.5">
      <NuxtLink :to="`/s/${product.shop_slug}`" class="truncate text-[11px] font-semibold uppercase tracking-wide text-muted hover:text-brand-500">{{ product.shop_name }}</NuxtLink>
      <NuxtLink :to="to" class="mt-0.5 line-clamp-2 text-sm font-semibold leading-snug hover:text-brand-500">{{ product.name }}</NuxtLink>
      <div v-if="veh" class="mt-1 text-xs text-muted">{{ veh.year }} · {{ km(veh.mileage_km) }} · {{ optLabel('transmission', veh.transmission) }}</div>
      <div class="mt-auto flex items-end justify-between gap-2 pt-3">
        <div class="text-base font-extrabold tracking-tight">{{ money(product.price_cents, product.currency) }}</div>
        <UButton color="neutral" variant="ghost" square v-if="veh && !cartable" :to="to" class="size-9 bg-field" :aria-label="$t('vehicle.showroom.viewDetails')" :title="$t('vehicle.showroom.viewDetails')"><AppIcon name="chevron-right" class="size-4" /></UButton>
        <button
          v-else
          class="grid size-9 shrink-0 place-items-center rounded-xl transition-all duration-200 disabled:opacity-40"
          :class="added ? 'bg-mint-500 text-white' : 'bg-brand-500/12 text-brand-600 hover:-translate-y-0.5 hover:bg-brand-500 hover:text-white hover:shadow-[0_8px_18px_-8px_var(--brand-500)]'"
          :disabled="product.stock <= 0"
          :aria-label="added ? $t('store.card.added') : $t('store.card.addToCart')"
          :title="added ? $t('store.card.added') : $t('store.card.addToCart')"
          @click="quickAdd"
        >
          <AppIcon :name="added ? 'check' : 'plus'" class="size-[18px]" :stroke="2.4" />
        </button>
      </div>
    </div>
  </div>
</template>
