<script setup lang="ts">
import type { Order, ShippingOption, CatalogProduct } from '~/utils/types'
import type { CartItem } from '~/composables/useCart'
import type { DeliveryChoice } from '~/utils/delivery'

const { lines, total, setQty, clear } = useCart()
const { loggedIn, user } = useAuth()
const api = useApi()
const { t } = useI18n()
const { carrierName } = useCarriers()
const addr = reactive({ name: '', phone: '', line1: '', branch: '', city: '', postcode: '' })
const error = ref('')
const busy = ref(false)
const placed = ref<Order[] | null>(null)
/** Supplier shop names of the placed orders (the cart is cleared after checkout). */
const placedShops = ref<Record<string, string>>({})
watchEffect(() => {
  if (user.value && !addr.name) addr.name = user.value.display_name
})

// Lines saved before delivery options existed don't know their supplier shop — look it up once.
const lookedUp = new Set<string>()
watch(
  () => lines.value.filter((l) => !l.shop_id).map((l) => l.product_id).join(','),
  async (missing) => {
    for (const id of missing.split(',').filter((x) => x && !lookedUp.has(x))) {
      lookedUp.add(id)
      try {
        const { product } = await api<{ product: CatalogProduct }>(`/catalog/products/${id}`)
        for (const l of lines.value) if (l.product_id === id) Object.assign(l, { shop_id: product.shop_id, supplier_name: product.shop_name })
      } catch {
        /* product gone — checkout will report it */
      }
    }
  },
  { immediate: true },
)

interface Group { shopId: string; name: string; currency: string; items: { line: CartItem; index: number }[]; subtotal: number }
const groups = computed<Group[]>(() => {
  const map = new Map<string, Group>()
  lines.value.forEach((line, index) => {
    const key = line.shop_id ?? ''
    let g = map.get(key)
    if (!g) map.set(key, (g = { shopId: key, name: line.supplier_name || line.shop_name, currency: line.currency, items: [], subtotal: 0 }))
    g.items.push({ line, index })
    g.subtotal += line.price_cents * line.qty
  })
  return [...map.values()]
})
const unresolved = computed(() => lines.value.some((l) => !l.shop_id))

// Delivery options per supplier shop.
const options = ref<Record<string, ShippingOption[]>>({})
const optionsLoading = ref(false)
const shopKey = computed(() => [...new Set(lines.value.map((l) => l.shop_id).filter(Boolean))].sort().join(','))
watch(
  shopKey,
  async (key) => {
    if (!key || !import.meta.client) return
    optionsLoading.value = true
    try {
      options.value = { ...options.value, ...(await api<Record<string, ShippingOption[]>>('/shipping/options', { query: { shops: key } })) }
    } catch (e) {
      error.value = apiError(e)
    } finally {
      optionsLoading.value = false
    }
  },
  { immediate: true },
)
const choices = ref<Record<string, DeliveryChoice | null>>({})

const summary = computed(() =>
  groups.value.map((g) => {
    const opts = options.value[g.shopId] ?? []
    const q = quoteChoice(opts, choices.value[g.shopId], g.subtotal)
    const shipping = q?.charged_fee_cents ?? 0
    const codFee = q?.cod_fee_cents ?? 0
    const shippingLabel = !q
      ? opts.length ? '—' : t('delivery.notNeeded')
      : q.fee_payer === 'seller' ? t('delivery.free')
      : q.fee_payer === 'destination' ? t('delivery.payAtPickup')
      : money(shipping, g.currency)
    return {
      ...g,
      hasOptions: opts.length > 0,
      q,
      shippingLabel,
      destinationFee: q?.fee_payer === 'destination' ? q.fee_cents : 0,
      codFee,
      total: g.subtotal + shipping + codFee,
      cod: !!q?.cod,
    }
  }),
)
const currency = computed(() => lines.value[0]?.currency ?? 'LAK')
const grandTotal = computed(() => summary.value.reduce((n, s) => n + s.total, 0))
const payNowTotal = computed(() => summary.value.filter((s) => !s.cod).reduce((n, s) => n + s.total, 0))
const codTotal = computed(() => summary.value.filter((s) => s.cod).reduce((n, s) => n + s.total, 0))
const anyCod = computed(() => summary.value.some((s) => s.cod))
const anyBranch = computed(() => summary.value.some((s) => s.q && choices.value[s.shopId]?.delivery_type === 'branch'))
const ready = computed(() => !unresolved.value && !optionsLoading.value && summary.value.every((s) => !s.hasOptions || s.q))

async function checkout() {
  if (!loggedIn.value) return navigateTo({ path: '/login', query: { next: '/cart' } })
  if (!ready.value) return (error.value = t('delivery.chooseEach'))
  error.value = ''
  busy.value = true
  try {
    const delivery = summary.value
      .filter((s) => s.hasOptions && s.q)
      .map((s) => {
        const c = choices.value[s.shopId]!
        return { shop_id: s.shopId, carrier_code: c.carrier_code, delivery_type: c.delivery_type, payment_method: s.cod ? 'cod' : 'prepaid' }
      })
    const names = Object.fromEntries(groups.value.map((g) => [g.shopId, g.name]))
    const address: Record<string, string> = { ...addr }
    if (!address.branch) delete address.branch
    placed.value = await api<Order[]>('/orders/checkout', {
      method: 'POST',
      body: {
        items: lines.value.map((l) => ({ product_id: l.product_id, qty: l.qty, via_shop_id: l.via_shop_id })),
        shipping_address: address,
        delivery,
      },
    })
    placedShops.value = names
    clear()
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}

const prepaid = computed(() => (placed.value ?? []).filter((o) => o.payment_method !== 'cod'))
const placedRows = computed(() =>
  (placed.value ?? []).map((o) => ({
    o,
    shop: placedShops.value[o.items[0]?.supplier_shop_id ?? ''] ?? '',
    courier: o.carrier_code ? carrierName(o.carrier_code) : '',
    type: o.carrier_code ? t(`delivery.${o.delivery_type}`) : '',
  })),
)
const prepaidTotal = computed(() => prepaid.value.reduce((n, o) => n + o.grand_total_cents, 0))

async function payAll() {
  busy.value = true
  error.value = ''
  try {
    for (const o of prepaid.value) if (o.status === 'pending') await api(`/orders/${o.id}/pay`, { method: 'POST' })
    await navigateTo('/orders')
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
useHead({ title: () => t('store.cart.title') + ' · zaokaiy' })
</script>

<template>
  <div class="mx-auto max-w-5xl px-4 pt-10">
    <h1 class="page-title text-3xl">{{ placed ? $t('store.cart.orderPlaced') : $t('store.cart.yourCart') }}</h1>
    <ClientOnly>
      <div v-if="placed" class="card mt-6 p-5 sm:p-6">
        <p>{{ $t('delivery.placedCount', placed.length) }}</p>
        <ul class="mt-4 divide-y divide-line rounded-2xl border border-line">
          <li v-for="r in placedRows" :key="r.o.id" class="flex flex-wrap items-start justify-between gap-2 p-4 text-sm">
            <div class="min-w-0">
              <div class="font-bold">#{{ shortId(r.o.id) }} <span v-if="r.shop" class="font-normal text-muted">· {{ r.shop }}</span></div>
              <div v-if="r.courier" class="text-xs text-muted">{{ r.courier }} · {{ r.type }}</div>
              <div v-if="r.o.payment_method === 'cod'" class="mt-1 text-xs font-semibold text-amber-800">{{ $t('delivery.codToCourier', { amount: money(r.o.cod_amount_cents, r.o.currency) }) }}</div>
              <div v-if="r.o.fee_payer === 'destination' && r.o.carrier_code" class="mt-1 text-xs text-muted">{{ $t('delivery.destinationNote', { fee: money(r.o.shipping_fee_cents, r.o.currency) }) }}</div>
            </div>
            <div class="text-right">
              <div class="font-bold tabular-nums">{{ money(r.o.grand_total_cents, r.o.currency) }}</div>
              <StatusBadge v-if="r.o.payment_method === 'cod'" status="confirmed" :label="$t('delivery.codConfirmed')" />
            </div>
          </li>
        </ul>
        <template v-if="prepaid.length">
          <p class="mt-4">{{ $t('delivery.dueNow') }} <b class="tabular-nums">{{ money(prepaidTotal, prepaid[0]?.currency) }}</b></p>
          <p class="mt-1 text-sm text-muted">{{ $t('store.cart.simulated') }}</p>
        </template>
        <p v-else class="mt-4 text-sm text-muted">{{ $t('delivery.allCod') }}</p>
        <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>
        <div class="mt-5 flex flex-wrap gap-3">
          <template v-if="prepaid.length">
            <UButton type="submit" :disabled="busy" @click="payAll">{{ $t('store.cart.payNow') }}</UButton>
            <UButton color="neutral" variant="soft" to="/orders">{{ $t('store.cart.payLater') }}</UButton>
          </template>
          <UButton v-else to="/orders">{{ $t('delivery.viewOrders') }}</UButton>
        </div>
      </div>

      <div v-else-if="lines.length" class="mt-6 grid gap-6 lg:grid-cols-[1fr_360px] lg:gap-8">
        <div class="space-y-4">
          <section v-for="s in summary" :key="s.shopId || 'unknown'" class="card overflow-hidden">
            <div class="border-b border-line bg-paper px-4 py-2.5 text-sm font-bold">{{ $t('delivery.shipsFrom', { shop: s.name }) }}</div>
            <div class="divide-y divide-line">
              <div v-for="{ line: l, index: i } in s.items" :key="`${l.product_id}-${l.via_shop_id}`" class="flex items-center gap-4 p-4">
                <ProductThumb :src="l.image" :name="l.name" class="size-20 shrink-0" rounded="rounded-xl" />
                <div class="min-w-0 flex-1">
                  <NuxtLink :to="{ path: `/p/${l.product_id}`, query: l.via_shop_id ? { via: l.via_shop_id } : {} }" class="font-semibold hover:underline">{{ l.name }}</NuxtLink>
                  <div class="text-xs text-muted">{{ $t('store.cart.from', { shop: l.shop_name }) }}</div>
                  <div class="mt-2 flex items-center gap-2">
                    <UButton color="neutral" variant="soft" size="sm" type="submit" @click="setQty(i, l.qty - 1)">−</UButton>
                    <span class="w-6 text-center text-sm font-semibold">{{ l.qty }}</span>
                    <UButton color="neutral" variant="soft" size="sm" type="submit" @click="setQty(i, l.qty + 1)">+</UButton>
                    <button class="ml-2 text-xs text-muted hover:text-brand-700" @click="setQty(i, 0)">{{ $t('common.remove') }}</button>
                  </div>
                </div>
                <div class="font-bold">{{ money(l.price_cents * l.qty, l.currency) }}</div>
              </div>
            </div>
            <div v-if="s.hasOptions" class="border-t border-line p-4">
              <div class="mb-2 text-sm font-bold">{{ $t('delivery.title') }}</div>
              <DeliveryPicker v-model="choices[s.shopId]" :options="options[s.shopId] ?? []" :currency="s.currency" :subtotal-cents="s.subtotal" />
            </div>
            <div v-else-if="optionsLoading || !s.shopId" class="border-t border-line p-4 text-xs text-muted">{{ $t('delivery.loading') }}</div>
          </section>
        </div>

        <form class="card h-fit space-y-3 p-5" @submit.prevent="checkout">
          <div class="font-bold">{{ $t('store.cart.shippingTitle') }}</div>
          <UInput v-model="addr.name" required :placeholder="$t('store.cart.fullName')" autocomplete="name" />
          <UInput v-model="addr.phone" required :placeholder="$t('common.phone')" inputmode="tel" autocomplete="tel" />
          <UInput v-model="addr.line1" required :placeholder="$t('common.address')" autocomplete="street-address" />
          <div class="flex gap-2">
            <UInput v-model="addr.city" required :placeholder="$t('store.cart.city')" />
            <UInput v-model="addr.postcode" required :placeholder="$t('store.cart.postcode')" />
          </div>
          <div>
            <UInput v-model="addr.branch" :placeholder="$t('delivery.pickupBranch')" />
            <p class="mt-1 text-xs" :class="anyBranch ? 'text-amber-800' : 'text-muted'">{{ $t('delivery.pickupBranchHint') }}</p>
          </div>

          <div class="space-y-3 border-t border-line pt-3 text-sm">
            <dl v-for="s in summary" :key="s.shopId || 'unknown'" class="space-y-0.5">
              <dt v-if="summary.length > 1" class="font-semibold">{{ s.name }}</dt>
              <div class="flex justify-between"><dt class="text-muted">{{ $t('common.subtotal') }}</dt><dd class="tabular-nums">{{ money(s.subtotal, s.currency) }}</dd></div>
              <div class="flex justify-between"><dt class="text-muted">{{ $t('delivery.shipping') }}</dt><dd class="tabular-nums" :class="s.q?.fee_payer === 'seller' ? 'text-mint-500' : ''">{{ s.shippingLabel }}</dd></div>
              <div v-if="s.codFee" class="flex justify-between"><dt class="text-muted">{{ $t('delivery.codFee') }}</dt><dd class="tabular-nums">{{ money(s.codFee, s.currency) }}</dd></div>
              <div v-if="summary.length > 1" class="flex justify-between font-semibold"><dt>{{ $t('common.total') }}</dt><dd class="tabular-nums">{{ money(s.total, s.currency) }}</dd></div>
              <p v-if="s.destinationFee" class="text-xs text-muted">{{ $t('delivery.destinationNote', { fee: money(s.destinationFee, s.currency) }) }}</p>
            </dl>
          </div>

          <div class="space-y-1 border-t border-line pt-3">
            <div class="flex justify-between font-bold"><span>{{ $t('delivery.grandTotal') }}</span><span class="tabular-nums">{{ money(grandTotal || total, currency) }}</span></div>
            <template v-if="anyCod">
              <div class="flex justify-between text-sm"><span>{{ $t('delivery.payNowTotal') }}</span><span class="tabular-nums">{{ money(payNowTotal, currency) }}</span></div>
              <div class="flex justify-between text-sm font-semibold text-amber-800"><span>{{ $t('delivery.payOnDeliveryTotal') }}</span><span class="tabular-nums">{{ money(codTotal, currency) }}</span></div>
              <p class="rounded-xl bg-sun-400/15 p-2.5 text-xs text-amber-900">{{ $t('delivery.codExplain', { now: money(payNowTotal, currency), later: money(codTotal, currency) }) }}</p>
            </template>
          </div>
          <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
          <UButton type="submit" class="w-full" :disabled="busy || (loggedIn && !ready)">{{ loggedIn ? (busy ? $t('store.cart.placing') : $t('store.cart.placeOrder')) : $t('store.cart.loginToCheckout') }}</UButton>
        </form>
      </div>

      <EmptyState v-else class="mt-6" :title="$t('store.cart.emptyTitle')" :text="$t('store.cart.emptyText')">
        <UButton to="/">{{ $t('store.cart.browse') }}</UButton>
      </EmptyState>
    </ClientOnly>
  </div>
</template>
