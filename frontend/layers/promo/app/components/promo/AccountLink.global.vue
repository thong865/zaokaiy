<script setup lang="ts">
/** Account page: link to "My rewards" with the number of coupons ready to use. */
const api = useApi()
const { data } = useLazyAsyncData('my-rewards-count', () => api<{ coupons: { status: string }[]; points: { balance: number }[] }>('/me/rewards').catch(() => null), { server: false })
const ready = computed(() => (data.value?.coupons ?? []).filter((c) => c.status === 'active').length)
const pts = computed(() => (data.value?.points ?? []).reduce((n, p) => n + p.balance, 0))
</script>

<template>
  <NuxtLink to="/rewards" class="card mt-6 flex items-center gap-4 p-5 hover:shadow-lg">
    <span class="grid size-11 place-items-center rounded-2xl bg-brand-500 text-white"><UIcon name="i-lucide-gift" class="size-5" /></span>
    <div class="min-w-0 flex-1">
      <div class="font-bold">{{ $t('promo.rewards.title') }}</div>
      <div class="text-sm text-muted">{{ $t('promo.rewards.summary', { coupons: ready, points: pts.toLocaleString() }) }}</div>
    </div>
    <UIcon name="i-lucide-chevron-right" class="size-5 text-muted" />
  </NuxtLink>
</template>
