<script setup lang="ts">
import type { Order } from '~/utils/types'

definePageMeta({ middleware: 'auth' })
const api = useApi()
const { data: orders, refresh } = await useAsyncData('my-orders', () => api<Order[]>('/orders'), { default: () => [] })
const error = ref('')
async function act(o: Order, action: 'pay' | 'cancel') {
  error.value = ''
  try {
    await api(`/orders/${o.id}/${action}`, { method: 'POST' })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
const steps = ['pending', 'paid', 'shipped', 'completed']
const { t } = useI18n()
useHead({ title: () => t('common.nav.myOrders') + ' · zaokaiy' })
</script>

<template>
  <div class="mx-auto max-w-4xl px-4 pt-10">
    <h1 class="page-title text-3xl">{{ $t('common.nav.myOrders') }}</h1>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>
    <div v-if="orders.length" class="mt-6 space-y-4">
      <div v-for="o in orders" :key="o.id" class="card p-5">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div>
            <div class="font-bold">#{{ shortId(o.id) }}</div>
            <div class="text-xs text-muted">{{ fmtDateTime(o.created_at) }}</div>
          </div>
          <StatusBadge :status="o.status" />
        </div>
        <div v-if="o.status !== 'cancelled'" class="mt-4 flex gap-1">
          <div v-for="s in steps" :key="s" class="h-1.5 flex-1 rounded-full" :class="steps.indexOf(o.status) >= steps.indexOf(s) ? 'bg-brand-500' : 'bg-line'" />
        </div>
        <ul class="mt-4 space-y-1 text-sm">
          <li v-for="it in o.items" :key="it.id" class="flex justify-between">
            <span>{{ it.product_name }} × {{ it.qty }}</span>
            <span>{{ money(it.unit_price_cents * it.qty, o.currency) }}</span>
          </li>
        </ul>
        <div class="mt-4 flex items-center justify-between border-t border-line pt-4">
          <span class="font-bold">{{ money(o.total_cents, o.currency) }}</span>
          <div class="flex gap-2">
            <NuxtLink v-if="o.status !== 'cancelled'" :to="`/print/order/${o.id}`" target="_blank" class="btn-ghost btn-sm">{{ $t('store.orders.receipt') }}</NuxtLink>
            <button v-if="o.status === 'pending'" class="btn-primary btn-sm" @click="act(o, 'pay')">{{ $t('store.orders.payDemo') }}</button>
            <button v-if="['pending', 'paid'].includes(o.status)" class="btn-ghost btn-sm" @click="act(o, 'cancel')">{{ $t('store.orders.cancel') }}</button>
          </div>
        </div>
      </div>
    </div>
    <EmptyState v-else class="mt-6" :title="$t('store.orders.emptyTitle')"><NuxtLink to="/" class="btn-primary">{{ $t('store.orders.startShopping') }}</NuxtLink></EmptyState>
  </div>
</template>
