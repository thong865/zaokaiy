<script setup lang="ts">
import type { CatalogProduct, Content, PublicMedia, ShippingOption, Shop, ShopContact, VehicleSpec } from '~/utils/types'

const route = useRoute()
const api = useApi()
const via = computed(() => (route.query.via as string) || undefined)
const { data, error } = await useAsyncData(`product:${route.params.id}:${via.value}`, () =>
  api<{ product: CatalogProduct; via_shop: Shop | null; contents: Content[]; media: PublicMedia[]; breadcrumb: { name: string; slug: string }[]; vehicle: VehicleSpec | null; contact: ShopContact | null }>(`/catalog/products/${route.params.id}`, {
    query: { via: via.value },
  }),
)
if (error.value || !data.value) throw createError({ statusCode: 404, statusMessage: tr('store.product.notFound'), fatal: true })

const p = computed(() => data.value!.product)
const qty = ref(1)
const { add } = useCart()
const added = ref(false)
function addToCart(buyNow = false) {
  add(
    {
      product_id: p.value.id,
      via_shop_id: p.value.via_shop_id,
      name: p.value.name,
      price_cents: p.value.price_cents,
      currency: p.value.currency,
      image: p.value.images?.[0] ?? null,
      shop_name: data.value!.via_shop?.name ?? p.value.shop_name,
      shop_id: p.value.shop_id,
      supplier_name: p.value.shop_name,
    },
    qty.value,
  )
  if (buyNow) return navigateTo('/cart')
  added.value = true
  setTimeout(() => (added.value = false), 1500)
}
// Supplier shop's couriers — client-side, non-blocking.
const { locale } = useI18n()
const { data: shipOpts } = useAsyncData(
  `ship-opts:${p.value.shop_id}`,
  () => api<Record<string, ShippingOption[]>>('/shipping/options', { query: { shops: p.value.shop_id } }).then((r) => r[p.value.shop_id] ?? []),
  { server: false, lazy: true, default: () => [] as ShippingOption[] },
)
// Vehicle listings: contact + request flow instead of (or next to) the cart.
const vehicle = computed(() => data.value?.vehicle ?? null)
const contact = computed(() => data.value?.contact ?? null)
const canBuyOnline = computed(() => !!vehicle.value?.buy_online && vehicle.value.sale_status === 'available' && p.value.stock > 0)
const leadKind = ref<string | undefined>()
const reqUrl = useRequestURL()
const waText = computed(() =>
  tr('vehicle.detail.waMessage', { name: p.value.name, price: money(p.value.price_cents, p.value.currency), url: `${reqUrl.origin}/p/${p.value.id}` }),
)
function pickLead(k: string) {
  leadKind.value = k
  document.getElementById('lead-title')?.scrollIntoView({ behavior: 'smooth', block: 'center' })
}
const couriers = computed(() => shipOpts.value.map((o) => optionName(o, locale.value)).join(' · '))
const codOffered = computed(() => shipOpts.value.some(codAvailable))
useHead({ title: () => `${p.value.name} · zaokaiy`, meta: [{ name: 'description', content: () => p.value.description.slice(0, 150) }] })
</script>

<template>
  <div v-if="data && vehicle" class="mx-auto grid max-w-6xl gap-8 px-4 pt-10 lg:grid-cols-[1fr_400px]">
    <div class="min-w-0 space-y-6">
      <ProductGallery :media="data.media ?? []" :name="p.name" />
      <VehicleSpecSheet :spec="vehicle" />
      <section class="card p-5">
        <h2 class="font-bold">{{ $t('common.description') }}</h2>
        <div class="mt-2 whitespace-pre-line text-sm text-ink/80">{{ p.description || $t('store.product.noDescription') }}</div>
      </section>
    </div>
    <div class="space-y-5 lg:sticky lg:top-24 lg:self-start">
      <div class="card p-5">
        <div v-if="data.via_shop" class="mb-3 rounded-xl bg-brand-50 px-3 py-2 text-xs">
          <i18n-t keypath="store.product.recommendedBy" tag="span"><template #shop><NuxtLink :to="`/s/${data.via_shop.slug}`" class="font-bold underline">{{ data.via_shop.name }}</NuxtLink></template><template #origin>{{ p.shop_name }}</template></i18n-t>
        </div>
        <div class="flex flex-wrap gap-1.5">
          <span class="chip font-bold" :class="vehicle.sale_status === 'available' ? 'bg-mint-500/15 text-mint-500' : vehicle.sale_status === 'sold' ? 'bg-ink text-paper' : 'bg-amber-600 text-white'">{{ optLabel('sale', vehicle.sale_status) }}</span>
          <span class="chip bg-paper">{{ optLabel('condition', vehicle.condition) }}</span>
          <span v-if="vehicle.finance_available" class="chip bg-paper">{{ $t('vehicle.showroom.financeBadge') }}</span>
        </div>
        <div class="mt-3 flex flex-wrap items-center gap-2"><NuxtLink :to="`/s/${p.shop_slug}`" class="text-sm font-semibold text-muted hover:text-ink">{{ p.shop_name }}</NuxtLink><KybVerifiedBadge v-if="p.shop_verified" /></div>
        <h1 class="text-2xl font-extrabold tracking-tight">{{ p.name }}</h1>
        <div class="mt-1 text-sm text-muted">{{ vehicleTitle(vehicle) }}</div>
        <div class="mt-3 text-3xl font-extrabold">{{ money(p.price_cents, p.currency) }}</div>
        <div class="text-xs text-muted">{{ vehicle.negotiable ? $t('vehicle.showroom.negotiable') : $t('vehicle.showroom.fixedPrice') }}</div>
        <div class="mt-4 grid grid-cols-2 gap-2 text-sm">
          <div class="rounded-xl bg-paper p-2.5"><div class="text-xs text-muted">{{ $t('vehicle.spec.year') }}</div><b>{{ vehicle.year }}</b></div>
          <div class="rounded-xl bg-paper p-2.5"><div class="text-xs text-muted">{{ $t('vehicle.spec.mileage') }}</div><b>{{ km(vehicle.mileage_km) }}</b></div>
          <div class="rounded-xl bg-paper p-2.5"><div class="text-xs text-muted">{{ $t('vehicle.spec.fuel') }}</div><b>{{ optLabel('fuel', vehicle.fuel) }}</b></div>
          <div class="rounded-xl bg-paper p-2.5"><div class="text-xs text-muted">{{ $t('vehicle.spec.transmission') }}</div><b>{{ optLabel('transmission', vehicle.transmission) }}</b></div>
        </div>
        <p v-if="vehicle.sale_status === 'reserved' && vehicle.reserved_until" class="mt-3 text-sm font-semibold text-amber-700">{{ $t('vehicle.detail.reservedUntil', { date: fmtDate(vehicle.reserved_until) }) }}</p>
        <p v-if="vehicle.sale_status === 'sold'" class="mt-3 text-sm font-semibold">{{ $t('vehicle.detail.sold') }}</p>
        <div v-if="vehicle.sale_status !== 'sold'" class="mt-4 grid gap-2">
          <div class="grid grid-cols-2 gap-2">
            <UButton color="neutral" v-if="contact && telLink(contact.phone)" :to="telLink(contact.phone)">📞 {{ $t('vehicle.detail.call') }}</UButton>
            <UButton color="neutral" variant="soft" v-if="contact && waLink(contact.phone, waText)" :to="waLink(contact.phone, waText)" target="_blank" rel="noopener">💬 {{ $t('vehicle.detail.whatsapp') }}</UButton>
          </div>
          <UButton type="submit" @click="pickLead('test_drive')">{{ $t('vehicle.lead.kinds.test_drive') }}</UButton>
          <UButton color="neutral" variant="soft" type="submit" v-if="vehicle.sale_status === 'available' && vehicle.deposit_cents > 0" @click="pickLead('reserve')">{{ $t('vehicle.detail.deposit', { amount: money(vehicle.deposit_cents, p.currency) }) }}</UButton>
          <UButton color="neutral" variant="soft" type="submit" v-if="canBuyOnline" @click="addToCart(true)">{{ $t('vehicle.detail.buyOnline') }}</UButton>
        </div>
        <div v-if="contact?.address" class="mt-4 text-xs text-muted">📍 {{ contact.address }}</div>
      </div>
      <VehicleLeadForm v-if="vehicle.sale_status !== 'sold'" :product-id="p.id" :product-name="p.name" :price="p.price_cents" :currency="p.currency" :spec="vehicle" :contact="contact" :initial-kind="leadKind" />
      <VehicleLoanCalculator v-if="vehicle.sale_status !== 'sold' && p.price_cents > 0" :price="p.price_cents" :currency="p.currency" />
    </div>
  </div>
  <div v-else-if="data" class="mx-auto grid max-w-6xl gap-10 px-4 pt-10 md:grid-cols-2">
    <ProductGallery :media="data.media ?? []" :name="p.name" />
    <div>
      <div v-if="data.via_shop" class="mb-4 flex items-center gap-2 rounded-2xl bg-brand-50 px-4 py-3 text-sm">
        <span>✦</span>
        <i18n-t keypath="store.product.recommendedBy" tag="span"><template #shop><NuxtLink :to="`/s/${data.via_shop.slug}`" class="font-bold underline">{{ data.via_shop.name }}</NuxtLink></template><template #origin>{{ p.shop_name }}</template></i18n-t>
      </div>
      <nav v-if="data.breadcrumb?.length" class="mb-2 flex flex-wrap items-center gap-1 text-xs text-muted" :aria-label="$t('store.product.breadcrumb')">
        <NuxtLink to="/" class="hover:text-ink">{{ $t('store.product.home') }}</NuxtLink>
        <template v-for="b in data.breadcrumb" :key="b.slug">
          <span>›</span><NuxtLink :to="{ path: '/', query: { category: b.slug } }" class="hover:text-ink">{{ b.name }}</NuxtLink>
        </template>
      </nav>
      <div class="flex flex-wrap items-center gap-2"><NuxtLink :to="`/s/${p.shop_slug}`" class="text-sm font-semibold text-muted hover:text-ink">{{ p.shop_name }}</NuxtLink><KybVerifiedBadge v-if="p.shop_verified" /></div>
      <h1 class="mt-1 text-3xl font-extrabold tracking-tight">{{ p.name }}</h1>
      <div class="mt-3 text-3xl font-extrabold">{{ money(p.price_cents, p.currency) }}</div>
      <div class="mt-2 text-sm" :class="p.stock > 0 ? 'text-mint-500' : 'text-brand-700'">
        {{ p.stock > 0 ? $t('store.product.inStock', { n: p.stock }) : $t('store.card.soldOut') }}
      </div>

      <div class="mt-6 flex items-center gap-3">
        <div class="flex items-center rounded-xl bg-field">
          <button class="px-4 py-2" @click="qty = Math.max(1, qty - 1)">−</button>
          <span class="w-8 text-center font-semibold">{{ qty }}</span>
          <button class="px-4 py-2" @click="qty = Math.min(p.stock, qty + 1)">+</button>
        </div>
        <UButton color="neutral" type="submit" class="flex-1" :disabled="p.stock <= 0" @click="addToCart()">{{ added ? $t('store.card.added') : $t('store.card.addToCart') }}</UButton>
        <UButton type="submit" class="flex-1" :disabled="p.stock <= 0" @click="addToCart(true)">{{ $t('store.product.buyNow') }}</UButton>
      </div>

      <div v-if="shipOpts.length" class="mt-4 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
        <span class="font-semibold">{{ $t('delivery.title') }}:</span>
        <span class="text-ink/80">{{ couriers }}</span>
        <span v-if="codOffered" class="chip bg-sun-400/20 text-amber-800">{{ $t('delivery.codAvailable') }}</span>
      </div>

      <div class="prose prose-sm mt-8 max-w-none whitespace-pre-line text-ink/80">{{ p.description || $t('store.product.noDescription') }}</div>

      <div v-if="p.allow_resell" class="card mt-8 p-5">
        <div class="font-bold">{{ $t('store.product.resellTitle', { pct: pct(p.commission_bps) }) }}</div>
        <p class="mt-1 text-sm text-muted">{{ $t('store.product.resellText', { shop: p.shop_name, amount: money(Math.floor(p.price_cents * p.commission_bps / 10000), p.currency) }) }}</p>
        <UButton color="neutral" variant="soft" size="sm" to="/dashboard/marketplace" class="mt-3">{{ $t('store.home.becomeSellStaff') }}</UButton>
      </div>
    </div>

    <section v-if="data.contents.length" class="md:col-span-2">
      <h2 class="text-xl font-extrabold tracking-tight">{{ $t('store.product.creatorsTalking') }}</h2>
      <div class="mt-4 grid gap-4 md:grid-cols-3">
        <article v-for="c in data.contents" :key="c.id" class="card p-5">
          <div class="text-[11px] font-bold uppercase tracking-wider text-muted">{{ kindLabel(c.kind) }}</div>
          <h3 class="mt-1 font-bold">{{ c.title }}</h3>
          <p class="mt-2 line-clamp-6 whitespace-pre-line text-sm text-ink/80">{{ c.body }}</p>
        </article>
      </div>
    </section>
  </div>
</template>
