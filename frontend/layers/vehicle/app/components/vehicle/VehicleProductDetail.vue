<script setup lang="ts">
/** Product page of a vehicle listing: gallery, spec sheet, contact + buyer requests, loan calculator. */
import type { CatalogProduct, PublicMedia, Shop, ShopContact, VehicleSpec } from '~/utils/types'

const props = defineProps<{
  data: { product: CatalogProduct; via_shop: Shop | null; media: PublicMedia[]; vehicle: VehicleSpec; contact: ShopContact | null }
}>()
const p = computed(() => props.data.product)
const vehicle = computed(() => props.data.vehicle)
const contact = computed(() => props.data.contact ?? null)
const canBuyOnline = computed(() => !!vehicle.value.buy_online && vehicle.value.sale_status === 'available' && p.value.stock > 0)
const leadKind = ref<string | undefined>()
const reqUrl = useRequestURL()
const waText = computed(() =>
  tr('vehicle.detail.waMessage', { name: p.value.name, price: money(p.value.price_cents, p.value.currency), url: `${reqUrl.origin}/p/${p.value.id}` }),
)
function pickLead(k: string) {
  leadKind.value = k
  document.getElementById('lead-title')?.scrollIntoView({ behavior: 'smooth', block: 'center' })
}
// "Buy online" (when the dealer allows it) goes through the normal cart + checkout.
const { add } = useCart()
function addToCart() {
  add(
    {
      product_id: p.value.id,
      via_shop_id: p.value.via_shop_id,
      name: p.value.name,
      price_cents: p.value.price_cents,
      currency: p.value.currency,
      image: p.value.images?.[0] ?? null,
      shop_name: props.data.via_shop?.name ?? p.value.shop_name,
      shop_id: p.value.shop_id,
      supplier_name: p.value.shop_name,
    },
    1,
  )
  return navigateTo('/cart')
}
</script>

<template>
  <div class="mx-auto grid max-w-6xl gap-8 px-4 pt-10 lg:grid-cols-[1fr_400px]">
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
          <UButton color="neutral" variant="soft" type="submit" v-if="canBuyOnline" @click="addToCart()">{{ $t('vehicle.detail.buyOnline') }}</UButton>
        </div>
        <div v-if="contact?.address" class="mt-4 text-xs text-muted">📍 {{ contact.address }}</div>
      </div>
      <VehicleLeadForm v-if="vehicle.sale_status !== 'sold'" :product-id="p.id" :product-name="p.name" :price="p.price_cents" :currency="p.currency" :spec="vehicle" :contact="contact" :initial-kind="leadKind" />
      <VehicleLoanCalculator v-if="vehicle.sale_status !== 'sold' && p.price_cents > 0" :price="p.price_cents" :currency="p.currency" />
    </div>
  </div>
</template>
