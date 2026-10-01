<script setup lang="ts">
import type { PosDoc } from '~/utils/types'

/** Thermal receipt page (80 mm or 58 mm) for the browser's print dialog. ?copies=2 prints two. */
definePageMeta({ colorMode: 'light', layout: 'print', middleware: 'auth' })
const route = useRoute()
const api = useApi()
const width = computed(() => (route.query.w === '58' ? 58 : 80))
const copies = computed(() => Math.min(Math.max(Number(route.query.copies) || 1, 1), 5))
const { data: doc } = await useAsyncData(`receipt:${route.params.id}`, () => api<PosDoc>(`/pos/sales/${route.params.id}`))
const { t } = useI18n()
useAutoPrint(computed(() => !!doc.value))
useHead({
  title: () => (doc.value ? t('pos.print.receiptNo', { number: doc.value.sale.number }) : t('pos.print.receipt')),
  style: [{ innerHTML: () => `@page { size: ${width.value}mm auto; margin: 0 } @media print { body { width: ${width.value}mm } .copy + .copy { break-before: page } }` }],
})
</script>

<template>
  <div v-if="doc">
    <div v-for="n in copies" :key="n" class="copy">
      <PosReceipt :doc="doc" :width="width" class="receipt-shadow" />
    </div>
    <div class="no-print mt-6 flex justify-center gap-2 pb-4 font-sans">
      <UButton color="neutral" size="sm" type="submit" onclick="window.print()">{{ $t('common.print') }}</UButton>
      <UButton color="neutral" variant="soft" size="sm" :to="{ query: { w: width === 80 ? '58' : '80' } }">{{ width === 80 ? '58 mm' : '80 mm' }}</UButton>
    </div>
  </div>
</template>

<style scoped>
@media screen { .receipt-shadow { box-shadow: 0 4px 20px rgb(0 0 0 / .15); } .copy + .copy { margin-top: 16px; } }
</style>
