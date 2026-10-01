<script setup lang="ts">
/** Home screen section: a few restaurants to order from. */
import type { Place } from '#restaurant/utils/restaurant'

const api = useApi()
const route = useRoute()
const { data: places } = await useAsyncData('home-places', () => api<Place[]>('/restaurant/places', { query: { limit: 3 } }).catch(() => []), { default: () => [] })
</script>

<template>
  <section v-if="places.length && !route.query.q && !route.query.category" class="mx-auto max-w-7xl px-4 pt-10">
    <div class="flex items-end justify-between gap-3">
      <h2 class="text-2xl font-extrabold tracking-tight">{{ $t('restaurant.places.homeTitle') }}</h2>
      <NuxtLink to="/food" class="text-sm font-semibold text-brand-600 hover:underline">{{ $t('restaurant.places.seeAll') }} →</NuxtLink>
    </div>
    <div class="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <RestaurantPlaceCard v-for="p in places" :key="p.id" :place="p" />
    </div>
  </section>
</template>
