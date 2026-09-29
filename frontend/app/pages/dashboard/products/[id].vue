<script setup lang="ts">
import type { MediaAsset, Product } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const route = useRoute()
const api = useApi()
const { shopId, shop } = useShop()
const { locale } = useI18n()
const { data: product, refresh } = await useAsyncData(`product-edit:${route.params.id}`, () => api<Product>(`/products/${route.params.id}`))
const { data: gallery } = await useAsyncData(`product-media:${route.params.id}`, () => api<MediaAsset[]>(`/products/${route.params.id}/media`), { default: () => [] })
const form = ref<{ f: { description: string } } | null>(null)
const busy = ref(false)
const msg = ref('')
const error = ref('')

async function save(body: Record<string, unknown>) {
  busy.value = true
  error.value = ''
  msg.value = ''
  try {
    product.value = await api<Product>(`/products/${route.params.id}`, { method: 'PATCH', body })
    msg.value = 'common.saved'
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}

const ai = reactive({ busy: false, tone: 'friendly and energetic', language: locale.value === 'lo' ? 'Lao' : 'English' })
const aiLanguages = ['English', 'Thai', 'Lao', 'Vietnamese', 'Chinese']
async function writeDescription() {
  ai.busy = true
  error.value = ''
  try {
    const r = await api<{ body: string }>(`/shops/${shopId.value}/ai/generate`, {
      method: 'POST',
      body: { product_id: route.params.id, kind: 'description', tone: ai.tone, language: ai.language },
    })
    if (form.value) form.value.f.description = r.body
  } catch (e) {
    error.value = apiError(e)
  } finally {
    ai.busy = false
  }
}

const stock = reactive({ delta: 10, reason: 'restock', note: '' })
async function adjust() {
  error.value = ''
  try {
    await api(`/products/${route.params.id}/stock`, { method: 'POST', body: { ...stock, delta: Number(stock.delta) } })
    await refresh()
    msg.value = 'product.edit.stockUpdated'
  } catch (e) {
    error.value = apiError(e)
  }
}
</script>

<template>
  <div v-if="product">
    <NuxtLink to="/dashboard/products" class="text-sm text-muted hover:text-ink">{{ $t('product.new.back') }}</NuxtLink>
    <div class="mb-6 mt-2 flex flex-wrap items-center gap-3">
      <h1 class="page-title">{{ product.name }}</h1>
      <StatusBadge :status="product.status" />
      <NuxtLink v-if="product.status === 'active' && product.review_status === 'approved'" :to="`/p/${product.id}`" target="_blank" class="ml-auto text-sm font-semibold text-brand-600 hover:underline">{{ $t('product.edit.viewLive') }}</NuxtLink>
    </div>
    <ReviewBanner :key="`${product.review_status}-${product.updated_at}`" class="mb-5" :product="product" @submitted="(p) => (product = p)" />
    <ProductForm :key="product.updated_at" ref="form" :product="product" :busy="busy" :currency="shop?.currency" :auto-approve="shop?.auto_approve" @submit="save">
      <template #media>
        <MediaGalleryEditor v-model="gallery" :shop-id="product.shop_id" :product-id="product.id" />
      </template>
      <template #description-tools>
        <div class="mb-1.5 flex items-center gap-2">
          <select v-model="ai.language" class="rounded-lg border border-line bg-surface px-2 py-1 text-xs" :aria-label="$t('product.edit.aiLanguage')">
            <option v-for="l in aiLanguages" :key="l" :value="l">{{ $t(`product.edit.langs.${l}`) }}</option>
          </select>
          <UButton color="neutral" size="sm" type="button" :disabled="ai.busy" @click="writeDescription">{{ ai.busy ? $t('product.edit.writing') : $t('product.edit.writeAi') }}</UButton>
        </div>
      </template>
      <template #after>
        <p v-if="msg" class="text-sm text-mint-500">{{ $t(msg) }}</p>
        <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
        <div class="card space-y-3 p-5">
          <div class="font-bold">{{ $t('product.edit.adjustStock') }}</div>
          <div class="flex gap-2">
            <UInput v-model.number="stock.delta" type="number" class="w-24" />
            <select v-model="stock.reason" class="input"><option value="restock">{{ $t('dash.inventory.reasons.restock') }}</option><option value="adjust">{{ $t('dash.inventory.reasons.adjust') }}</option><option value="return">{{ $t('dash.inventory.reasons.return') }}</option></select>
          </div>
          <UInput v-model="stock.note" :placeholder="$t('product.edit.notePh')" />
          <UButton color="neutral" variant="soft" type="button" class="w-full" @click="adjust">{{ $t('product.edit.apply') }}</UButton>
        </div>
        <UButton color="neutral" variant="soft" :to="{ path: '/dashboard/content', query: { product: product.id } }" class="w-full">{{ $t('product.edit.createContent') }}</UButton>
      </template>
    </ProductForm>
  </div>
</template>
