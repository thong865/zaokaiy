<script setup lang="ts">
import type { MediaAsset, Product } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId, shop } = useShop()
const busy = ref(false)
const gallery = ref<MediaAsset[]>([])
const error = ref('')
async function save(body: Record<string, unknown>) {
  busy.value = true
  error.value = ''
  try {
    const p = await api<Product>(`/shops/${shopId.value}/products`, { method: 'POST', body: { ...body, media_ids: gallery.value.map((a) => a.id) } })
    await navigateTo(`/dashboard/products/${p.id}`)
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div>
    <NuxtLink to="/dashboard/products" class="text-sm text-muted hover:text-ink">{{ $t('product.new.back') }}</NuxtLink>
    <h1 class="page-title mt-2">{{ $t('product.new.title') }}</h1>
    <p class="mb-6 text-sm text-muted">{{ $t('product.new.tip') }}</p>
    <ProductForm :busy="busy" :currency="shop?.currency" :auto-approve="shop?.auto_approve" @submit="save">
      <template #media>
        <MediaGalleryEditor v-if="shop" v-model="gallery" :shop-id="shop.id" />
      </template>
      <template #after><p v-if="error" class="text-sm text-brand-700">{{ error }}</p></template>
    </ProductForm>
  </div>
</template>
