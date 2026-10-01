<script setup lang="ts">
import type { Product } from '~/utils/types'

/** Barcode labels. ?items=<productId>:<copies>,…&layout=roll40x30|roll50x30|a4&price=1&name=1 */
definePageMeta({ colorMode: 'light', layout: 'print', middleware: 'auth' })
const route = useRoute()
const api = useApi()
const { shop } = useShop()
const layout = computed(() => (['roll40x30', 'roll50x30', 'a4'].includes(route.query.layout as string) ? (route.query.layout as string) : 'roll40x30'))
const showPrice = computed(() => route.query.price !== '0')
const showName = computed(() => route.query.name !== '0')
const wanted = computed(() =>
  String(route.query.items || '')
    .split(',')
    .map((s) => s.split(':'))
    .filter(([id]) => id)
    .map(([id, n]) => ({ id: id!, copies: Math.min(Math.max(Number(n) || 1, 1), 500) })),
)
// Load the products directly (a freshly opened print tab may not know the current shop yet).
const { data: products } = await useAsyncData(
  `labels:${route.query.items}`,
  () => Promise.all([...new Set(wanted.value.map((w) => w.id))].map((id) => api<Product>(`/products/${id}`).catch(() => null))).then((r) => r.filter((p): p is Product => !!p)),
  { default: () => [] as Product[] },
)
const { data: myShops } = await useAsyncData('labels:shops', () => api<{ id: string; currency: string }[]>('/me/shops'), { default: () => [] })
const labels = computed(() => {
  const out: Product[] = []
  for (const w of wanted.value) {
    const p = products.value.find((x) => x.id === w.id)
    if (p?.barcode) for (let i = 0; i < w.copies; i++) out.push(p)
  }
  return out
})
const currencyOf = (p: Product) => myShops.value.find((s) => s.id === p.shop_id)?.currency ?? shop.value?.currency ?? 'THB'
useAutoPrint(computed(() => labels.value.length > 0))
const PAGE: Record<string, string> = {
  roll40x30: '@page { size: 40mm 30mm; margin: 0 }',
  roll50x30: '@page { size: 50mm 30mm; margin: 0 }',
  a4: '@page { size: A4; margin: 10.7mm 0 0 0 }',
}
const { t } = useI18n()
useHead({ title: () => t('barcode.title'), style: [{ innerHTML: () => PAGE[layout.value]! }] })
</script>

<template>
  <div>
    <div class="no-print mx-auto mb-4 flex max-w-3xl items-center gap-2 px-4 font-sans">
      <UButton color="neutral" size="sm" type="submit" onclick="window.print()">{{ $t('common.print') }}</UButton>
      <UButton color="neutral" variant="soft" size="sm" to="/dashboard/barcodes">{{ $t('common.back') }}</UButton>
      <span class="text-sm text-muted">{{ $t('barcode.labelCount', { n: labels.length }, labels.length) }}</span>
    </div>
    <div :class="['sheet', layout]">
      <div v-for="(p, i) in labels" :key="i" class="bc-label">
        <div v-if="showName" class="name">{{ p.name }}</div>
        <div v-if="showPrice" class="price">{{ money(p.price_cents, currencyOf(p)) }}</div>
        <BarcodeSvg :value="p.barcode!" :height="40" class="bars" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.sheet { font-family: 'Plus Jakarta Sans', 'Noto Sans Lao', 'Noto Sans Thai', system-ui, sans-serif; color: #000; }
.bc-label { box-sizing: border-box; display: flex; flex-direction: column; align-items: center; justify-content: center; overflow: hidden; background: #fff; text-align: center; }
.name { font-weight: 700; line-height: 1.1; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; width: 100%; }
.price { font-weight: 800; }
.bars { width: 100%; }

/* Label-printer rolls: one label per page. */
.roll40x30 .bc-label { width: 40mm; height: 30mm; padding: 1.5mm 2mm; break-after: page; page-break-after: always; }
.roll50x30 .bc-label { width: 50mm; height: 30mm; padding: 1.5mm 2.5mm; break-after: page; page-break-after: always; }
.roll40x30 .name, .roll50x30 .name { font-size: 7pt; }
.roll40x30 .price, .roll50x30 .price { font-size: 9pt; margin: 0.3mm 0; }
.roll40x30 .bars, .roll50x30 .bars { height: 13mm; }

/* A4 sheet of 3 × 8 labels (70 × 37 mm, e.g. Avery 3474 / L7160-style). */
.a4 { display: grid; grid-template-columns: repeat(3, 70mm); grid-auto-rows: 37mm; width: 210mm; }
.a4 .bc-label { width: 70mm; height: 37mm; padding: 3mm 5mm; }
.a4 .name { font-size: 9pt; }
.a4 .price { font-size: 12pt; margin: 0.5mm 0; }
.a4 .bars { height: 17mm; }

@media screen {
  .sheet { margin: 0 auto; display: flex; flex-wrap: wrap; gap: 6px; justify-content: center; max-width: 900px; }
  .sheet.a4 { display: grid; gap: 0; outline: 1px solid #ccc; background: #fff; }
  .bc-label { outline: 1px dashed #bbb; }
}
</style>
