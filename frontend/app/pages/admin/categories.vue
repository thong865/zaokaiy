<script setup lang="ts">
import type { CategoryNode } from '~/utils/types'

definePageMeta({ layout: 'admin', middleware: 'admin' })
const api = useApi()
const { data: nodes, refresh } = await useAsyncData('admin-categories', () => api<CategoryNode[]>('/admin/categories'), { default: () => [] })
async function changed() {
  await refresh()
  clearNuxtData('marketplace-categories')
}
</script>

<template>
  <div class="max-w-4xl">
    <h1 class="page-title">{{ $t('admin.categories.title') }}</h1>
    <p class="text-sm text-muted">{{ $t('admin.categories.intro') }}</p>
    <CategoryTreeManager class="mt-6" scope="marketplace" create-path="/admin/categories" :nodes="nodes" @changed="changed" />
  </div>
</template>
