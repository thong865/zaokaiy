<script setup lang="ts">
/** Customer's order tracking page (/r/o/<token>): kitchen progress, refreshed every 10 seconds. */
import type { FoodOrder } from '#restaurant/utils/restaurant'

const route = useRoute()
const api = useApi()
const { data: order, refresh, error } = await useAsyncData(`track:${route.params.token}`, () => api<FoodOrder>(`/restaurant/track/${route.params.token}`))
if (error.value || !order.value) throw createError({ statusCode: 404, statusMessage: tr('restaurant.track.notFound'), fatal: true })

const steps = computed(() => (order.value?.mode === 'dine_in' ? ['new', 'accepted', 'preparing', 'ready', 'served'] : ['new', 'accepted', 'preparing', 'ready', 'completed']))
const at = computed(() => steps.value.indexOf(order.value!.status === 'completed' && order.value!.mode === 'dine_in' ? 'served' : order.value!.status))
const done = computed(() => ['completed', 'cancelled'].includes(order.value!.status))
let timer: ReturnType<typeof setInterval> | undefined
onMounted(() => {
  timer = setInterval(() => !done.value && refresh(), 10_000)
})
onBeforeUnmount(() => clearInterval(timer))
useHead({ title: () => `#${order.value?.number} · ${order.value?.shop?.name}` })
</script>

<template>
  <div v-if="order" class="mx-auto max-w-xl px-4 py-10">
    <div class="card p-6 text-center">
      <div class="text-sm font-semibold text-muted">{{ order.shop?.name }}</div>
      <div class="mt-1 text-5xl font-extrabold tracking-tight" data-testid="ticket">#{{ order.number }}</div>
      <div class="mt-1 text-sm text-muted">
        {{ $t(`restaurant.mode.${order.mode}`) }}<template v-if="order.table_label"> · {{ $t('restaurant.menu.table', { label: order.table_label }) }}</template>
      </div>
      <p v-if="order.status === 'cancelled'" class="mt-4 rounded-xl bg-brand-50 p-3 font-semibold text-brand-700" role="status">{{ $t('restaurant.track.cancelled') }}</p>
      <ol v-else class="mt-6 grid grid-cols-5 gap-1 text-[11px] font-semibold" :aria-label="$t('restaurant.track.progress')">
        <li v-for="(s, i) in steps" :key="s" class="flex flex-col items-center gap-1.5" :class="i <= at ? 'text-brand-600' : 'text-muted'" :aria-current="i === at ? 'step' : undefined">
          <span class="h-1.5 w-full rounded-full" :class="i <= at ? 'bg-brand-500' : 'bg-line'" />
          {{ $t(`restaurant.status.${s}`) }}
        </li>
      </ol>
      <p v-if="!done && order.shop?.prep_minutes" class="mt-4 text-sm text-muted">{{ $t('restaurant.track.eta', { n: order.shop.prep_minutes }) }}</p>
    </div>
    <div class="card mt-4 p-5">
      <ul class="space-y-2 text-sm">
        <li v-for="l in order.items" :key="l.id" class="flex gap-2">
          <b>{{ l.qty }} ×</b><span class="flex-1">{{ l.name }}<span v-if="l.note" class="block text-xs text-muted">{{ l.note }}</span></span><span>{{ money(l.qty * l.unit_price_cents, order.currency) }}</span>
        </li>
      </ul>
      <div class="mt-3 space-y-1 border-t border-line pt-3 text-sm">
        <div v-if="order.service_cents" class="flex justify-between text-muted"><span>{{ $t('restaurant.order.serviceShort') }}</span><span>{{ money(order.service_cents, order.currency) }}</span></div>
        <div class="flex justify-between text-base font-extrabold"><span>{{ $t('common.total') }}</span><span>{{ money(order.total_cents, order.currency) }}</span></div>
        <div class="text-right text-xs" :class="order.paid ? 'text-mint-500' : 'text-muted'">{{ order.paid ? $t('restaurant.track.paid') : $t('restaurant.track.payAtCounter') }}</div>
      </div>
    </div>
    <div class="mt-4 flex flex-wrap justify-center gap-2">
      <UButton v-if="order.shop" color="neutral" variant="soft" :to="`/s/${order.shop.slug}`">{{ $t('restaurant.track.backToMenu') }}</UButton>
      <UButton v-if="order.shop && telLink(order.shop.phone)" color="neutral" variant="soft" :to="telLink(order.shop.phone)">📞 {{ $t('restaurant.track.call') }}</UButton>
    </div>
  </div>
</template>
