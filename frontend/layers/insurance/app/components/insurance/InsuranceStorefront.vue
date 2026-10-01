<script setup lang="ts">
/** Storefront of an insurance agent (/s/<slug>): the agent's plans. */
import type { Shop } from '~/utils/types'
import type { Plan } from '#insurance/utils/insurance'

const props = defineProps<{ shop: Shop }>()
const api = useApi()
const { data: plans } = await useAsyncData(`agent-store:${props.shop.slug}`, () => api<Plan[]>('/insurance/plans', { query: { shop: props.shop.slug } }), { default: () => [] })
</script>

<template>
  <div class="mx-auto max-w-7xl px-4 pt-10">
    <div class="card flex flex-wrap items-center gap-5 p-6">
      <ProductThumb :src="shop.logo_url" :name="shop.name" class="size-20" rounded="rounded-3xl" />
      <div class="min-w-0 flex-1">
        <div class="flex flex-wrap items-center gap-2">
          <h1 class="text-2xl font-extrabold tracking-tight">{{ shop.name }}</h1>
          <span class="chip bg-violet-500/12 text-violet-600">{{ $t('common.verticals.insurance') }}</span>
          <KybVerifiedBadge v-if="shop.kyb_verified_at" />
        </div>
        <p class="mt-1 text-sm text-muted">{{ shop.description }}</p>
        <p v-if="shop.address" class="mt-2 text-xs text-muted">📍 {{ shop.address }}</p>
      </div>
      <UButton v-if="telLink(shop.phone)" color="neutral" :to="telLink(shop.phone)">📞 {{ shop.phone }}</UButton>
    </div>
    <div v-if="plans.length" class="mt-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <InsurancePlanCard v-for="p in plans" :key="p.id" :plan="p" />
    </div>
    <EmptyState v-else class="mt-6" :title="$t('insurance.public.emptyTitle')" :text="$t('insurance.public.emptyText')" />
  </div>
</template>
