<script setup lang="ts">
definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const { shopId, shop } = useShop()
const { data: nodes, refresh } = await useShopCategories()
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('category.page.title') }}</h1>
    <i18n-t keypath="category.page.intro" tag="p" class="text-sm text-muted">
      <template #storefront>
        <NuxtLink v-if="shop" :to="`/s/${shop.slug}`" target="_blank" class="font-semibold text-ink underline">{{ $t('category.page.yourStorefront') }}</NuxtLink>
        <template v-else>{{ $t('category.page.yourStorefront') }}</template>
      </template>
    </i18n-t>
    <CategoryTreeManager v-if="shopId" class="mt-6" scope="shop" :shop-id="shopId" :create-path="`/shops/${shopId}/categories`" :nodes="nodes" @changed="refresh()" />
  </div>
</template>
