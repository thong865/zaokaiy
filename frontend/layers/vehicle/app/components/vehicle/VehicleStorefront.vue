<script setup lang="ts">
/** Storefront of a vehicle shop (/s/<slug>): dealer header + searchable showroom. */
import type { Shop } from '~/utils/types'

const props = defineProps<{ shop: Shop }>()
const waText = computed(() => tr('vehicle.showroom.waHello', { shop: props.shop.name }))
</script>

<template>
  <div class="mx-auto max-w-7xl px-4 pt-10">
    <div class="card flex flex-wrap items-center gap-5 bg-night p-6 text-white">
      <ProductThumb :src="shop.logo_url" :name="shop.name" class="size-20" rounded="rounded-3xl" />
      <div class="min-w-0 flex-1">
        <div class="flex flex-wrap items-center gap-2">
          <h1 class="text-2xl font-extrabold tracking-tight">{{ shop.name }}</h1>
          <span class="chip bg-white/15 text-white">{{ $t('vehicle.store.vehicle') }}</span>
          <KybVerifiedBadge v-if="shop.kyb_verified_at" dark />
        </div>
        <p class="mt-1 max-w-2xl text-sm text-white/75">{{ shop.description || $t('store.shop.welcome') }}</p>
        <p v-if="shop.address" class="mt-2 text-xs text-white/70">📍 {{ shop.address }}</p>
      </div>
      <div class="flex flex-wrap gap-2">
        <UButton color="neutral" variant="ghost" size="sm" v-if="telLink(shop.phone)" :to="telLink(shop.phone)" class="rounded-xl bg-white px-4 font-semibold text-night">📞 {{ $t('vehicle.detail.call') }}</UButton>
        <UButton color="neutral" variant="ghost" size="sm" v-if="waLink(shop.phone, waText)" :to="waLink(shop.phone, waText)" target="_blank" rel="noopener" class="rounded-xl bg-mint-500 px-4 font-semibold text-white">💬 {{ $t('vehicle.detail.whatsapp') }}</UButton>
      </div>
    </div>
    <div class="mt-8"><VehicleShowroom :shop-slug="shop.slug" /></div>
  </div>
</template>
