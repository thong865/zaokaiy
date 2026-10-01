<script setup lang="ts">
/** Insurance plans from agents on zaokaiy, filtered by type. */
import { KIND_ICON, PLAN_KINDS, type Plan } from '#insurance/utils/insurance'

const api = useApi()
const route = useRoute()
const kind = computed(() => (route.query.kind as string) || '')
const { data: plans } = await useAsyncData('ins-plans', () => api<Plan[]>('/insurance/plans', { query: { kind: kind.value || undefined } }), {
  watch: [kind],
  default: () => [],
})
useHead({ title: () => `${tr('common.superApp.insurance')} · zaokaiy` })
</script>

<template>
  <div class="mx-auto max-w-7xl px-4 pt-10">
    <h1 class="page-title text-3xl">{{ $t('insurance.public.title') }}</h1>
    <p class="mt-1 text-sm text-muted">{{ $t('insurance.public.subtitle') }}</p>
    <div class="mt-5 flex flex-wrap gap-2">
      <NuxtLink to="/insurance" class="rounded-xl px-3.5 py-1.5 text-sm font-semibold" :class="!kind ? 'bg-brand-500 text-white' : 'bg-surface shadow-card hover:text-brand-500'">{{ $t('common.all') }}</NuxtLink>
      <NuxtLink v-for="k in PLAN_KINDS" :key="k" :to="{ path: '/insurance', query: { kind: k } }" class="flex items-center gap-1.5 rounded-xl px-3.5 py-1.5 text-sm font-semibold" :class="kind === k ? 'bg-brand-500 text-white' : 'bg-surface shadow-card hover:text-brand-500'">
        <UIcon :name="`i-lucide-${KIND_ICON[k]}`" class="size-4" />{{ $t(`insurance.kind.${k}`) }}
      </NuxtLink>
    </div>
    <div v-if="plans.length" class="mt-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <InsurancePlanCard v-for="p in plans" :key="p.id" :plan="p" />
    </div>
    <EmptyState v-else class="mt-6" :title="$t('insurance.public.emptyTitle')" :text="$t('insurance.public.emptyText')" />
    <p class="mt-8 text-xs text-muted">{{ $t('insurance.public.disclaimer') }}</p>
  </div>
</template>
