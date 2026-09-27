<script setup lang="ts">
import type { Product } from '~/utils/types'

const props = defineProps<{ product?: Product | null; busy?: boolean; currency?: string; autoApprove?: boolean }>()
const emit = defineEmits<{ submit: [body: Record<string, unknown>] }>()
const { t } = useI18n()

const f = reactive({
  sku: props.product?.sku ?? '',
  name: props.product?.name ?? '',
  description: props.product?.description ?? '',
  price: props.product ? props.product.price_cents / 100 : 0,
  barcode: props.product?.barcode ?? '',
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
const perUnit = computed(() => Math.floor(Number(f.price) * 100 * Number(f.commission) / 100))
defineExpose({ f })
</script>

<template>
  <form class="grid gap-6 lg:grid-cols-[1fr_320px]" @submit.prevent="submit">
    <div class="card space-y-4 p-6">
      <div class="grid gap-4 sm:grid-cols-[1fr_180px]">
        <div><label class="label">{{ $t('common.name') }}</label><input v-model="f.name" required class="input" ></div>
        <div><label class="label">{{ $t('product.form.sku') }}</label><input v-model="f.sku" :disabled="!!product" required class="input disabled:bg-paper" ></div>
      </div>
      <div>
        <div class="flex items-center justify-between"><label class="label">{{ $t('common.description') }}</label><slot name="description-tools" /></div>
        <textarea v-model="f.description" rows="8" class="input" />
      </div>
      <slot name="media" />
      <div class="grid gap-4 sm:grid-cols-2">
        <div><label class="label">{{ $t('product.form.price', { currency: currency ?? 'THB' }) }}</label><input v-model="f.price" type="number" min="0" step="0.01" required class="input" ></div>
        <div>
          <label class="label">{{ $t('common.statusLabel') }}</label>
          <select v-model="f.status" class="input"><option value="draft">{{ $t('product.form.statusDraft') }}</option><option value="active">{{ $t('product.form.statusActive') }}</option><option value="archived">{{ $t('common.status.archived') }}</option></select>
        </div>
      </div>
    </div>
    <div class="space-y-4">
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
        <div v-if="!product"><label class="label">{{ $t('product.form.initialStock') }}</label><input v-model="f.initial_stock" type="number" min="0" class="input" ></div>
        <div v-else class="text-sm">{{ $t('product.form.inStock') }} <b>{{ product.stock }}</b> <NuxtLink to="/dashboard/inventory" class="text-brand-600 underline">{{ $t('product.form.adjust') }}</NuxtLink></div>
        <div>
          <label class="label">{{ $t('product.form.socialCode') }}</label>
          <input v-model="f.social_code" maxlength="10" class="input font-mono uppercase" :placeholder="$t('product.form.socialCodePh')" >
          <p class="mt-1 text-xs text-muted">{{ $t('product.form.socialHint', { code: f.social_code || 'A01' }) }}</p>
        </div>
        <div><label class="label">{{ $t('product.form.barcode') }}</label><input v-model="f.barcode" class="input font-mono" :placeholder="$t('product.form.barcodePh')" ></div>
        <div><label class="label">{{ $t('product.form.lowStockAt') }}</label><input v-model="f.low_stock_threshold" type="number" min="0" class="input" ></div>
      </div>
      <div class="card space-y-4 p-5">
        <label class="flex items-center justify-between gap-3">
          <span><span class="font-bold">{{ $t('product.form.openResell') }}</span><br><span class="text-xs text-muted">{{ $t('product.form.openResellHint') }}</span></span>
          <input v-model="f.allow_resell" type="checkbox" class="size-5 accent-brand-500" >
        </label>
        <div v-if="f.allow_resell">
          <label class="label">{{ $t('product.form.defaultCommission') }}</label>
          <input v-model="f.commission" type="number" min="0" max="90" step="0.5" class="input" >
          <p class="mt-1 text-xs text-muted">{{ $t('product.form.perUnit', { amount: money(perUnit, currency) }) }}</p>
        </div>
      </div>
      <button class="btn-primary w-full" :disabled="busy || publishBlocked">{{ submitLabel }}</button>
      <p v-if="publishBlocked" class="-mt-2 text-center text-xs text-brand-700">{{ $t('product.form.publishBlocked') }}</p>
      <p v-else-if="reviewNeeded && product?.review_status === 'approved'" class="-mt-2 text-center text-xs text-amber-700">{{ $t('product.form.reviewLive') }}</p>
      <p v-else-if="reviewNeeded" class="-mt-2 text-center text-xs text-muted">{{ $t('product.form.reviewNew') }}</p>
      <slot name="after" />
    </div>
  </form>
</template>
