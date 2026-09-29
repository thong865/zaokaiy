<script setup lang="ts">
import type { Product, VehicleDraft, VehicleSpec } from '~/utils/types'

const props = defineProps<{ product?: Product | null; busy?: boolean; currency?: string; autoApprove?: boolean }>()
const emit = defineEmits<{ submit: [body: Record<string, unknown>] }>()
const { t } = useI18n()
const route = useRoute()
const api = useApi()
const { shopId, shop } = useShop()

const f = reactive({
  sku: props.product?.sku ?? '',
  name: props.product?.name ?? '',
  description: props.product?.description ?? '',
  price: props.product ? props.product.price_cents / 100 : 0,
  // New product opened from an unknown POS scan: /dashboard/products/new?barcode=…
  barcode: props.product?.barcode ?? (typeof route.query.barcode === 'string' ? route.query.barcode : ''),
  social_code: props.product?.social_code ?? '',
  category_id: props.product?.category_id ?? null,
  shop_category_id: props.product?.shop_category_id ?? null,
  status: props.product?.status ?? 'active',
  initial_stock: 0,
  low_stock_threshold: props.product?.low_stock_threshold ?? 5,
  allow_resell: props.product?.allow_resell ?? false,
  commission: (props.product?.commission_bps ?? 1000) / 100,
})
watch(() => props.product?.description, (d) => { if (d !== undefined) f.description = d })

// Vehicle listing: spec sheet stored next to the product (vehicle dealers get it by default).
const spec = props.product?.id
  ? await api<VehicleSpec | null>(`/products/${props.product.id}/vehicle`).catch(() => null)
  : null
const isVehicle = ref(!!spec || (!props.product && shop.value?.vertical === 'vehicle'))
const toDraft = (s: VehicleSpec | null): VehicleDraft => ({
  vehicle_type: s?.vehicle_type ?? 'car', condition: s?.condition ?? 'used', make: s?.make ?? '', model: s?.model ?? '',
  variant: s?.variant ?? '', year: s?.year ?? '', mileage_km: s?.mileage_km ?? 0, fuel: s?.fuel ?? 'petrol',
  transmission: s?.transmission ?? 'automatic', body_type: s?.body_type ?? '', drive: s?.drive ?? '',
  engine_cc: s?.engine_cc ?? '', power_hp: s?.power_hp ?? '', seats: s?.seats ?? '', doors: s?.doors ?? '',
  color: s?.color ?? '', owners: s?.owners ?? '', registered: s?.registered ?? true, plate_province: s?.plate_province ?? '',
  plate_no: s?.plate_no ?? '', vin: s?.vin ?? '', location: s?.location ?? '', features: [...(s?.features ?? [])],
  warranty_months: s?.warranty_months ?? '', deposit: s ? s.deposit_cents / 100 : 0, negotiable: s?.negotiable ?? true,
  finance_available: s?.finance_available ?? false, buy_online: s?.buy_online ?? false, sale_status: s?.sale_status ?? 'available',
})
const vehicle = ref<VehicleDraft>(toDraft(spec))
if (isVehicle.value && !props.product) f.initial_stock = 1
const optNum = (x: string | number | null) => (x === '' || x === null || x === undefined ? null : Number(x))
function vehicleBody() {
  const { deposit, ...v } = vehicle.value
  return {
    ...v,
    year: Number(v.year) || null,
    mileage_km: Number(v.mileage_km) || 0,
    engine_cc: optNum(v.engine_cc), power_hp: optNum(v.power_hp), seats: optNum(v.seats), doors: optNum(v.doors),
    owners: optNum(v.owners), warranty_months: optNum(v.warranty_months),
    deposit_cents: Math.round(Number(deposit || 0) * 100),
    sale_status: props.product ? v.sale_status : undefined,
  }
}
const canonMake = (m: string) => [...CAR_MAKES, ...BIKE_MAKES].find((k) => k.toLowerCase() === m.trim().toLowerCase()) ?? m.trim()
const suggestedName = computed(() => (vehicle.value.make && vehicle.value.model && vehicle.value.year
  ? vehicleTitle({ make: canonMake(vehicle.value.make), model: vehicle.value.model.trim(), variant: vehicle.value.variant.trim(), year: Number(vehicle.value.year) })
  : ''))

// Barcode helpers: camera scan, validation, in-store code generation.
const scanBarcode = ref(false)
const barcodeState = computed(() => {
  const c = f.barcode.trim()
  if (!c) return null
  if (/^\d+$/.test(c) && [8, 12, 13, 14].includes(c.length)) {
    if (gtinValid(c)) return { ok: true, text: t('product.form.barcodeValid') }
    return { ok: false, text: t('product.form.barcodeBadCheck', { digit: gtinCheckDigit(c.slice(0, -1)) }) }
  }
  return { ok: true, text: t('product.form.barcodeInternal') }
})
const generating = ref(false)
async function generateBarcode() {
  if (props.product?.id && shopId.value) {
    generating.value = true
    try {
      const r = await api<{ assigned: { id: string; barcode: string }[] }>(`/shops/${shopId.value}/barcodes/generate`, { method: 'POST', body: { product_ids: [props.product.id] } })
      if (r.assigned[0]) f.barcode = r.assigned[0].barcode
    } finally {
      generating.value = false
    }
    return
  }
  // New product: random in-store EAN-13 (prefix 20); uniqueness is enforced on save.
  const body = '20' + Array.from({ length: 10 }, () => Math.floor(Math.random() * 10)).join('')
  f.barcode = body + gtinCheckDigit(body)
}

function submit() {
  const body: Record<string, unknown> = {
    name: f.name,
    description: f.description,
    price_cents: Math.round(Number(f.price) * 100),
    ...(f.category_id ? { category_id: f.category_id } : {}),
    shop_category_id: f.shop_category_id || null,
    barcode: f.barcode.trim(),
    social_code: f.social_code.trim().toUpperCase(),
    status: f.status,
    low_stock_threshold: Number(f.low_stock_threshold),
    allow_resell: f.allow_resell,
    commission_bps: Math.round(Number(f.commission) * 100),
  }
  if (isVehicle.value) body.vehicle = vehicleBody()
  if (!props.product) {
    body.sku = f.sku
    body.initial_stock = Number(f.initial_stock)
  }
  emit('submit', body)
}
const { data: marketCats } = await useMarketplaceCategories()
const { data: shopCats } = await useShopCategories()

// Warn before an edit sends a live product back to the review queue.
const reviewNeeded = computed(() => {
  if (props.autoApprove) return false
  if (!props.product) return f.status === 'active'
  const p = props.product
  if (f.status !== 'active') return false
  if (p.review_status === 'not_submitted' || p.review_status === 'rejected') return true
  return p.review_status === 'approved' && (f.name.trim() !== p.name || f.description !== p.description || f.category_id !== p.category_id)
})
const publishBlocked = computed(() => f.status === 'active' && !f.category_id && props.product?.review_status !== 'approved')
const submitLabel = computed(() => {
  if (props.busy) return t('common.saving')
  if (!props.product) return f.status === 'active' ? (props.autoApprove ? t('product.form.createPublish') : t('product.form.createSubmit')) : t('product.form.saveDraft')
  return reviewNeeded.value ? t('product.form.saveSubmit') : t('product.form.saveChanges')
})
// Business shops publish only after verification (drafts are fine).
const kybGate = computed(() => shop.value?.entity_type === 'business' && !shop.value.kyb_verified_at && (!props.product || props.product.review_status !== 'approved'))
const perUnit = computed(() => Math.floor(Number(f.price) * 100 * Number(f.commission) / 100))
defineExpose({ f })
</script>

<template>
  <form class="grid gap-6 lg:grid-cols-[1fr_320px] lg:items-start" @submit.prevent="submit">
    <div class="card space-y-4 p-6 lg:col-start-1">
      <div class="grid gap-4 sm:grid-cols-[1fr_180px]">
        <div><label class="label">{{ $t('common.name') }}</label><UInput v-model="f.name" required /></div>
        <div><label class="label">{{ $t('product.form.sku') }}</label><UInput v-model="f.sku" :disabled="!!product" required /></div>
      </div>
      <div>
        <div class="flex items-center justify-between"><label class="label">{{ $t('common.description') }}</label><slot name="description-tools" /></div>
        <UTextarea v-model="f.description" :rows="8" />
      </div>
      <slot name="media" />
      <div v-if="isVehicle && suggestedName && f.name.trim() !== suggestedName" class="-mt-1 text-sm">
        <button type="button" class="font-semibold text-brand-600 hover:underline" @click="f.name = suggestedName">{{ $t('vehicle.form.useTitle', { name: suggestedName }) }}</button>
      </div>
      <div class="grid gap-4 sm:grid-cols-2">
        <div><label class="label">{{ $t('product.form.price', { currency: currency ?? 'THB' }) }}</label><UInput v-model.number="f.price" type="number" min="0" step="0.01" required /></div>
        <div>
          <label class="label">{{ $t('common.statusLabel') }}</label>
          <select v-model="f.status" class="input"><option value="draft">{{ $t('product.form.statusDraft') }}</option><option value="active">{{ $t('product.form.statusActive') }}</option><option value="archived">{{ $t('common.status.archived') }}</option></select>
        </div>
      </div>
    </div>
    <div v-if="isVehicle || !product || shop?.vertical === 'vehicle'" class="card space-y-4 p-6 lg:col-start-1">
      <label v-if="!spec" class="flex items-center justify-between gap-3">
        <span class="font-bold">{{ $t('vehicle.form.isVehicle') }}</span>
        <input v-model="isVehicle" type="checkbox" class="size-5 accent-brand-500">
      </label>
      <div v-else class="font-bold">{{ $t('vehicle.form.section') }}</div>
      <VehicleSpecForm v-if="isVehicle" v-model="vehicle" :currency="currency" :existing="!!product" />
    </div>
    <div class="space-y-4 lg:col-start-2 lg:row-span-2 lg:row-start-1">
      <div class="card space-y-4 p-5">
        <div class="font-bold">{{ $t('product.form.categories') }}</div>
        <div>
          <label class="label">{{ $t('product.form.marketCat') }} <span class="normal-case tracking-normal text-brand-600">{{ $t('product.form.requiredToPublish') }}</span></label>
          <CategorySelect v-model="f.category_id" :nodes="marketCats" require-leaf />
        </div>
        <div>
          <div class="flex items-center justify-between">
            <label class="label">{{ $t('product.form.shopCat') }}</label>
            <NuxtLink to="/dashboard/categories" class="mb-1.5 text-xs font-semibold text-brand-600 hover:underline">{{ $t('product.form.manage') }}</NuxtLink>
          </div>
          <select v-model="f.shop_category_id" class="input">
            <option :value="null">{{ $t('product.form.none') }}</option>
            <option v-for="c in shopCats" :key="c.id" :value="c.id">{{ '\u00a0\u00a0\u00a0'.repeat(c.depth) }}{{ c.depth ? '└ ' : '' }}{{ c.name }}</option>
          </select>
          <p v-if="!shopCats.length" class="mt-1 text-xs text-muted">{{ $t('product.form.shopCatHint') }}</p>
        </div>
      </div>
      <div class="card space-y-4 p-5">
        <div class="font-bold">{{ $t('product.form.inventory') }}</div>
        <div v-if="!product"><label class="label">{{ $t('product.form.initialStock') }}</label><UInput v-model.number="f.initial_stock" type="number" min="0" /></div>
        <div v-else class="text-sm">{{ $t('product.form.inStock') }} <b>{{ product.stock }}</b> <NuxtLink to="/dashboard/inventory" class="text-brand-600 underline">{{ $t('product.form.adjust') }}</NuxtLink></div>
        <div>
          <label class="label">{{ $t('product.form.socialCode') }}</label>
          <UInput v-model="f.social_code" maxlength="10" class="font-mono uppercase" :placeholder="$t('product.form.socialCodePh')" />
          <p class="mt-1 text-xs text-muted">{{ $t('product.form.socialHint', { code: f.social_code || 'A01' }) }}</p>
        </div>
        <div>
          <label class="label">{{ $t('product.form.barcode') }}</label>
          <div class="flex gap-2">
            <UInput v-model="f.barcode" class="font-mono" :placeholder="$t('product.form.barcodePh')" @keydown.enter.prevent />
            <UButton color="neutral" variant="soft" size="sm" type="button" class="shrink-0" :title="$t('pos.scan.camera')" @click="scanBarcode = true">📷</UButton>
            <UButton color="neutral" variant="soft" size="sm" v-if="!product?.barcode || !f.barcode" type="button" class="shrink-0" :disabled="generating || !!f.barcode" @click="generateBarcode">{{ $t('product.form.barcodeGenerate') }}</UButton>
          </div>
          <p v-if="barcodeState" class="mt-1 text-xs" :class="barcodeState.ok ? 'text-muted' : 'font-semibold text-brand-700'">{{ barcodeState.text }}</p>
          <BarcodeSvg v-if="f.barcode && barcodeState?.ok" :value="f.barcode.trim()" :height="36" class="mt-2 h-14 w-48" />
          <BarcodeCameraScanner v-if="scanBarcode" :continuous="false" @detected="(c) => (f.barcode = c)" @close="scanBarcode = false" />
        </div>
        <div><label class="label">{{ $t('product.form.lowStockAt') }}</label><UInput v-model.number="f.low_stock_threshold" type="number" min="0" /></div>
      </div>
      <div class="card space-y-4 p-5">
        <label class="flex items-center justify-between gap-3">
          <span><span class="font-bold">{{ $t('product.form.openResell') }}</span><br><span class="text-xs text-muted">{{ $t('product.form.openResellHint') }}</span></span>
          <input v-model="f.allow_resell" type="checkbox" class="size-5 accent-brand-500" >
        </label>
        <div v-if="f.allow_resell">
          <label class="label">{{ $t('product.form.defaultCommission') }}</label>
          <UInput v-model.number="f.commission" type="number" min="0" max="90" step="0.5" />
          <p class="mt-1 text-xs text-muted">{{ $t('product.form.perUnit', { amount: money(perUnit, currency) }) }}</p>
        </div>
      </div>
      <div v-if="kybGate && f.status === 'active'" class="rounded-2xl bg-sun-400/20 p-3 text-sm">
        {{ $t('kyb.gate.form') }} <NuxtLink to="/dashboard/verification" class="font-semibold text-brand-600 hover:underline">{{ $t('kyb.nav') }} →</NuxtLink>
      </div>
      <UButton type="submit" class="w-full" :disabled="busy || publishBlocked">{{ submitLabel }}</UButton>
      <p v-if="publishBlocked" class="-mt-2 text-center text-xs text-brand-700">{{ $t('product.form.publishBlocked') }}</p>
      <p v-else-if="reviewNeeded && product?.review_status === 'approved'" class="-mt-2 text-center text-xs text-amber-700">{{ $t('product.form.reviewLive') }}</p>
      <p v-else-if="reviewNeeded" class="-mt-2 text-center text-xs text-muted">{{ $t('product.form.reviewNew') }}</p>
      <slot name="after" />
    </div>
  </form>
</template>
