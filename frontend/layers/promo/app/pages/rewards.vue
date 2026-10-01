<script setup lang="ts">
import type { CouponView } from '../utils/promo'
import { rewardText } from '../utils/promo'

/** Customer: my coupons (sign-up gifts, first-order rewards) and my points at each shop. */
definePageMeta({ middleware: 'auth' })
const api = useApi()
const { t } = useI18n()
useHead({ title: () => t('promo.rewards.title') })
interface Points { shop_id: string; shop_name: string; shop_slug: string; logo_url: string | null; currency: string; balance: number; value_cents: number; enabled: boolean }
const { data } = await useAsyncData('my-rewards', () => api<{ phone: string | null; coupons: CouponView[]; points: Points[] }>('/me/rewards'), { server: false })
const active = computed(() => (data.value?.coupons ?? []).filter((c) => c.status === 'active'))
const past = computed(() => (data.value?.coupons ?? []).filter((c) => c.status !== 'active'))
const copied = ref('')
async function copy(code: string) {
  await navigator.clipboard?.writeText(code)
  copied.value = code
  setTimeout(() => (copied.value = ''), 1500)
}
</script>

<template>
  <div class="mx-auto max-w-3xl px-4 py-10">
    <h1 class="page-title">{{ $t('promo.rewards.title') }}</h1>
    <p class="mt-1 text-sm text-muted">{{ $t('promo.rewards.intro') }}</p>

    <h2 class="mt-6 font-bold">{{ $t('promo.rewards.coupons') }}</h2>
    <div v-if="active.length" class="mt-3 grid gap-3 sm:grid-cols-2">
      <article v-for="c in active" :key="c.id" class="card relative overflow-hidden p-4">
        <div class="absolute inset-y-0 left-0 w-1.5 bg-brand-500" />
        <div class="text-lg font-extrabold text-brand-700">{{ rewardText(t, c) }}</div>
        <div class="text-sm font-semibold">{{ c.campaign }}</div>
        <p v-if="c.description" class="mt-0.5 text-xs text-muted">{{ c.description }}</p>
        <div class="mt-2 text-xs text-muted">
          <template v-if="c.shop_name">{{ $t('promo.rewards.atShop') }} <NuxtLink :to="`/s/${c.shop_slug}`" class="font-semibold text-ink hover:underline">{{ c.shop_name }}</NuxtLink></template>
          <template v-else>{{ $t('promo.rewards.anyShop') }}</template>
          <template v-if="c.min_spend_cents"> · {{ $t('promo.minSpendShort', { amount: money(c.min_spend_cents, c.currency) }) }}</template>
          · {{ $t('promo.rewards.until', { date: fmtDate(c.expires_at) }) }}
        </div>
        <button class="mt-3 flex w-full items-center justify-between rounded-xl border border-dashed border-brand-500 px-3 py-2 font-mono font-bold" @click="copy(c.code)">
          {{ c.code }} <span class="font-sans text-xs font-semibold text-brand-700">{{ copied === c.code ? $t('common.copied') : $t('common.copy') }}</span>
        </button>
      </article>
    </div>
    <EmptyState v-else-if="data" class="mt-3" icon="i-lucide-ticket" :title="$t('promo.rewards.noCoupons')" :text="$t('promo.rewards.noCouponsText')" />

    <h2 class="mt-8 font-bold">{{ $t('promo.rewards.points') }}</h2>
    <p v-if="data && !data.phone" class="mt-1 rounded-xl bg-sun-400/15 p-2.5 text-xs text-amber-900">
      {{ $t('promo.rewards.needPhone') }} <NuxtLink to="/account" class="font-semibold underline">{{ $t('promo.rewards.connectWhatsapp') }}</NuxtLink>
    </p>
    <ul v-if="data?.points.length" class="card mt-3 divide-y divide-line/70">
      <li v-for="p in data.points" :key="p.shop_id" class="flex items-center gap-3 p-4">
        <ProductThumb :src="p.logo_url" :name="p.shop_name" class="size-10" rounded="rounded-xl" />
        <NuxtLink :to="`/s/${p.shop_slug}`" class="min-w-0 flex-1 font-semibold hover:underline">{{ p.shop_name }}</NuxtLink>
        <div class="text-right"><div class="text-lg font-extrabold tabular-nums">{{ p.balance.toLocaleString() }}</div><div class="text-xs text-muted">{{ $t('promo.rewards.worth', { amount: money(p.value_cents, p.currency) }) }}</div></div>
      </li>
    </ul>
    <p v-else-if="data?.phone" class="mt-2 text-sm text-muted">{{ $t('promo.rewards.noPoints') }}</p>

    <template v-if="past.length">
      <h2 class="mt-8 font-bold text-muted">{{ $t('promo.rewards.past') }}</h2>
      <ul class="mt-2 space-y-1 text-sm text-muted">
        <li v-for="c in past" :key="c.id">{{ rewardText(t, c) }} · {{ c.campaign }} · {{ $t(`promo.couponStatus.${c.status}`) }}</li>
      </ul>
    </template>
  </div>
</template>
