<script setup lang="ts">
import type { SaleRecord } from '~/utils/types'
import { receiptCanvas } from '~/utils/receipt'

const props = defineProps<{ sale: SaleRecord }>()
const { lang, settings } = usePos()
const src = ref('')
async function draw() {
  await document.fonts.ready
  src.value = receiptCanvas(props.sale, lang.value, settings.value.printer.paper).toDataURL('image/png')
}
watch(() => [props.sale, lang.value, settings.value.printer.paper], draw, { immediate: true })
</script>

<template>
  <div class="grid max-h-[60vh] justify-center overflow-auto rounded-xl bg-surface-2 p-4">
    <img v-if="src" :src="src" alt="" class="w-72 bg-white shadow-card" />
  </div>
</template>
