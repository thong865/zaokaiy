<script setup lang="ts">
/** All restaurants (open ones first), with search. */
import type { Place } from '#restaurant/utils/restaurant'

const api = useApi()
const q = ref('')
const { data: places, status } = await useAsyncData('places', () => api<Place[]>('/restaurant/places', { query: { q: q.value || undefined, limit: 60 } }), {
  default: () => [],
})
let t: ReturnType<typeof setTimeout> | undefined
watch(q, () => {
  clearTimeout(t)
  t = setTimeout(() => refreshNuxtData('places'), 300)
})
useHead({ title: () => `${tr('common.superApp.food')} · zaokaiy` })
</script>

<template>
  <div class="mx-auto max-w-7xl px-4 pt-10">
    <div class="flex flex-wrap items-end justify-between gap-4">
      <div>
        <h1 class="page-title text-3xl">{{ $t('restaurant.places.title') }}</h1>
        <p class="mt-1 text-sm text-muted">{{ $t('restaurant.places.subtitle') }}</p>
      </div>
      <UInput v-model="q" icon="i-lucide-search" class="w-full max-w-xs" :placeholder="$t('restaurant.places.search')" :aria-label="$t('common.search')" />
    </div>
    <div v-if="status === 'pending' && !places.length" class="mt-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <div v-for="i in 3" :key="i" class="skeleton aspect-[16/11] rounded-[20px]" />
    </div>
    <div v-else-if="places.length" class="mt-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <RestaurantPlaceCard v-for="p in places" :key="p.id" :place="p" />
    </div>
    <EmptyState v-else class="mt-6" :title="$t('restaurant.places.emptyTitle')" :text="$t('restaurant.places.emptyText')" />
  </div>
</template>
