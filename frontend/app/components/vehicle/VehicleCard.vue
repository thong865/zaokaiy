<script setup lang="ts">
import type { VehicleCard } from '~/utils/types'

const props = defineProps<{ v: VehicleCard; showShop?: boolean }>()
const to = computed(() => ({ path: `/p/${props.v.id}`, query: props.v.via_shop_id ? { via: props.v.via_shop_id } : {} }))
const facts = computed(() => [
  String(props.v.year),
  km(props.v.mileage_km),
  optLabel('fuel', props.v.fuel),
  optLabel('transmission', props.v.transmission),
])
</script>

<template>
  <NuxtLink :to="to" class="group card card-hover block overflow-hidden" :class="v.sale_status === 'sold' && 'opacity-70'">
    <div class="relative overflow-hidden">
      <ProductThumb :image="v.cover" :src="v.images?.[0]" :name="v.name" rounded="rounded-none" sizes="(min-width: 1280px) 25vw, (min-width: 768px) 33vw, 100vw" class="aspect-[4/3] transition-transform duration-500 group-hover:scale-[1.04]" />
      <div class="absolute left-3 top-3 flex flex-wrap gap-1.5">
        <span v-if="v.sale_status !== 'available'" class="chip font-bold text-white" :class="v.sale_status === 'sold' ? 'bg-night' : 'bg-amber-600'">{{ optLabel('sale', v.sale_status) }}</span>
        <span v-if="v.condition !== 'used'" class="chip bg-mint-500 font-bold text-white">{{ optLabel('condition', v.condition) }}</span>
      </div>
      <span v-if="v.finance_available" class="chip absolute right-3 top-3 bg-surface/90 font-semibold text-ink">{{ $t('vehicle.showroom.financeBadge') }}</span>
    </div>
    <div class="p-4">
      <div class="text-xs font-semibold uppercase tracking-wide text-muted">{{ v.make }} · {{ optLabel('body', v.body_type) || optLabel('type', v.vehicle_type) }}</div>
      <h3 class="mt-0.5 line-clamp-1 font-bold group-hover:underline">{{ v.name }}</h3>
      <div class="mt-2 flex flex-wrap gap-1.5 text-xs">
        <span v-for="f in facts" :key="f" class="rounded-lg bg-paper px-2 py-1 font-medium text-ink/80">{{ f }}</span>
      </div>
      <div class="mt-3 flex items-end justify-between gap-2">
        <div>
          <div class="text-lg font-extrabold leading-tight">{{ money(v.price_cents, v.currency) }}</div>
          <div class="text-xs text-muted">{{ v.negotiable ? $t('vehicle.showroom.negotiable') : $t('vehicle.showroom.fixedPrice') }}</div>
        </div>
        <div class="min-w-0 text-right text-xs text-muted">
          <div v-if="v.location" class="truncate">📍 {{ v.location }}</div>
          <div v-if="showShop" class="truncate">{{ v.shop_name }}</div>
        </div>
      </div>
    </div>
  </NuxtLink>
</template>
