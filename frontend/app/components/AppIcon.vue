<script setup lang="ts">
/** App icon names → Lucide icons rendered by Nuxt UI's <UIcon> (bundled locally via @iconify-json/lucide). */
const props = defineProps<{ name: string; stroke?: number }>()
const attrs = useAttrs()
const MAP: Record<string, string> = {
  bag: 'shopping-bag', cart: 'shopping-cart', dashboard: 'layout-dashboard', file: 'file-text', barcode: 'scan-barcode',
  message: 'message-circle', pen: 'pen-line', settings: 'sliders-horizontal', shield: 'shield-check', logout: 'log-out',
  external: 'external-link', 'check-circle': 'circle-check', trending: 'trending-up', calculator: 'calculator', store: 'store',
  folder: 'folder-tree', archive: 'archive', receipt: 'receipt', inbox: 'inbox', film: 'clapperboard',
}
const icon = computed(() => `i-lucide-${MAP[props.name] ?? props.name}`)
defineOptions({ inheritAttrs: false })
</script>

<template>
  <UIcon :name="icon" v-bind="attrs" :class="[attrs.class ? '' : 'size-5', 'shrink-0']" aria-hidden="true" />
</template>
