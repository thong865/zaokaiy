<script setup lang="ts">
import type { Order } from '~/utils/types'

const { lines, total, setQty, clear } = useCart()
const { loggedIn, user } = useAuth()
const api = useApi()
const addr = reactive({ name: '', phone: '', line1: '', city: '', postcode: '' })
const error = ref('')
const busy = ref(false)
const placed = ref<Order[] | null>(null)
watchEffect(() => {
  if (user.value && !addr.name) addr.name = user.value.display_name
})

async function checkout() {
  if (!loggedIn.value) return navigateTo({ path: '/login', query: { next: '/cart' } })
  error.value = ''
  busy.value = true
  try {
    placed.value = await api<Order[]>('/orders/checkout', {
      method: 'POST',
      body: {
        items: lines.value.map((l) => ({ product_id: l.product_id, qty: l.qty, via_shop_id: l.via_shop_id })),
        shipping_address: { ...addr },
      },
    })
    clear()
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}

async function payAll() {
  busy.value = true
  try {
    for (const o of placed.value ?? []) await api(`/orders/${o.id}/pay`, { method: 'POST' })
    await navigateTo('/orders')
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
const { t } = useI18n()
useHead({ title: () => t('store.cart.title') + ' · zaokaiy' })
</script>

<template>
  <div class="mx-auto max-w-5xl px-4 pt-10">
    <h1 class="page-title text-3xl">{{ placed ? $t('store.cart.orderPlaced') : $t('store.cart.yourCart') }}</h1>
    <ClientOnly>
      <div v-if="placed" class="card mt-6 p-6">
        <p>{{ $t('store.cart.created', placed.length) }}
          <b>{{ money(placed.reduce((n, o) => n + o.total_cents, 0), placed[0]?.currency) }}</b></p>
        <p class="mt-1 text-sm text-muted">{{ $t('store.cart.simulated') }}</p>
        <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>
        <div class="mt-5 flex gap-3">
          <button class="btn-primary" :disabled="busy" @click="payAll">{{ $t('store.cart.payNow') }}</button>
          <NuxtLink to="/orders" class="btn-ghost">{{ $t('store.cart.payLater') }}</NuxtLink>
        </div>
      </div>

      <div v-else-if="lines.length" class="mt-6 grid gap-8 lg:grid-cols-[1fr_360px]">
        <div class="card divide-y divide-line">
          <div v-for="(l, i) in lines" :key="`${l.product_id}-${l.via_shop_id}`" class="flex items-center gap-4 p-4">
            <ProductThumb :src="l.image" :name="l.name" class="size-20 shrink-0" rounded="rounded-xl" />
            <div class="min-w-0 flex-1">
              <NuxtLink :to="{ path: `/p/${l.product_id}`, query: l.via_shop_id ? { via: l.via_shop_id } : {} }" class="font-semibold hover:underline">{{ l.name }}</NuxtLink>
              <div class="text-xs text-muted">{{ $t('store.cart.from', { shop: l.shop_name }) }}</div>
              <div class="mt-2 flex items-center gap-2">
                <button class="btn-ghost btn-sm" @click="setQty(i, l.qty - 1)">−</button>
                <span class="w-6 text-center text-sm font-semibold">{{ l.qty }}</span>
                <button class="btn-ghost btn-sm" @click="setQty(i, l.qty + 1)">+</button>
                <button class="ml-2 text-xs text-muted hover:text-brand-700" @click="setQty(i, 0)">{{ $t('common.remove') }}</button>
              </div>
            </div>
            <div class="font-bold">{{ money(l.price_cents * l.qty, l.currency) }}</div>
          </div>
        </div>
        <form class="card h-fit space-y-3 p-5" @submit.prevent="checkout">
          <div class="font-bold">{{ $t('store.cart.shippingTitle') }}</div>
          <input v-model="addr.name" required :placeholder="$t('store.cart.fullName')" class="input" >
          <input v-model="addr.phone" required :placeholder="$t('common.phone')" class="input" >
          <input v-model="addr.line1" required :placeholder="$t('common.address')" class="input" >
          <div class="flex gap-2">
            <input v-model="addr.city" required :placeholder="$t('store.cart.city')" class="input" >
            <input v-model="addr.postcode" required :placeholder="$t('store.cart.postcode')" class="input" >
          </div>
          <div class="flex justify-between border-t border-line pt-3 font-bold">
            <span>{{ $t('common.total') }}</span><span>{{ money(total, lines[0]?.currency) }}</span>
          </div>
          <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
          <button class="btn-primary w-full" :disabled="busy">{{ loggedIn ? (busy ? $t('store.cart.placing') : $t('store.cart.placeOrder')) : $t('store.cart.loginToCheckout') }}</button>
        </form>
      </div>

      <EmptyState v-else class="mt-6" :title="$t('store.cart.emptyTitle')" :text="$t('store.cart.emptyText')">
        <NuxtLink to="/" class="btn-primary">{{ $t('store.cart.browse') }}</NuxtLink>
      </EmptyState>
    </ClientOnly>
  </div>
</template>
