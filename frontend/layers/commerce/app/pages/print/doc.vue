<script setup lang="ts">
import type { PosDoc } from '~/utils/types'

/** Prints a receipt handed over by the POS in sessionStorage (unpaid bill preview, test page). */
definePageMeta({ colorMode: 'light', layout: 'print' })
const route = useRoute()
const width = computed(() => (route.query.w === '58' ? 58 : 80))
const data = ref<{ doc: PosDoc; preview: boolean } | null>(null)
onMounted(() => {
  try {
    data.value = JSON.parse(sessionStorage.getItem('zk_print_doc') || 'null')
  } catch {}
})
useAutoPrint(computed(() => !!data.value))
useHead({ style: [{ innerHTML: () => `@page { size: ${width.value}mm auto; margin: 0 } @media print { body { width: ${width.value}mm } }` }] })
</script>

<template>
  <PosReceipt v-if="data" :doc="data.doc" :width="width" :preview="data.preview" />
</template>
