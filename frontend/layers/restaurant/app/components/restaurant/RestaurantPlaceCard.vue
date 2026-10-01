<script setup lang="ts">
import type { Place } from '#restaurant/utils/restaurant'

defineProps<{ place: Place }>()
</script>

<template>
  <NuxtLink :to="`/s/${place.slug}`" class="group card card-hover block overflow-hidden" :data-testid="`place-${place.slug}`">
    <div class="relative grid aspect-[16/9] grid-cols-3 gap-0.5 overflow-hidden bg-[var(--field)]">
      <ProductThumb v-for="(img, i) in (place.images.length ? place.images : [null]).slice(0, 3)" :key="i" :src="img" :name="place.name" rounded="rounded-none" class="h-full" :class="place.images.length <= 1 && 'col-span-3'" />
      <span v-if="!place.open" class="chip absolute left-3 top-3 bg-night/80 text-white">{{ $t('restaurant.places.closed') }}</span>
    </div>
    <div class="p-4">
      <div class="flex items-center gap-2"><h3 class="truncate font-bold group-hover:text-brand-500">{{ place.name }}</h3><KybVerifiedBadge v-if="place.verified" /></div>
      <p class="mt-0.5 line-clamp-1 text-xs text-muted">{{ place.description }}</p>
      <div class="mt-2 flex flex-wrap gap-x-3 text-xs text-muted">
        <span>⏱ {{ $t('restaurant.menu.prep', { n: place.prep_minutes }) }}</span>
        <span v-if="place.from_cents != null">{{ $t('restaurant.places.from', { price: money(place.from_cents, place.currency) }) }}</span>
        <span v-if="place.delivery">🛵 {{ $t('restaurant.mode.delivery') }}</span>
      </div>
    </div>
  </NuxtLink>
</template>
