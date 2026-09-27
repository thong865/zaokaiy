<script setup lang="ts">
/** End-of-day (Z) report on receipt paper. ?shop=<id>&date=YYYY-MM-DD&w=80|58 */
import type { DocKey } from '~/utils/docLabels'

definePageMeta({ layout: 'print', middleware: 'auth' })
const route = useRoute()
const api = useApi()
const width = computed(() => (route.query.w === '58' ? 58 : 80))
interface Summary {
  date: string; generated_at: string; shop: { name: string; legal_name: string; tax_id: string; branch: string; address: string; currency: string }
  sales: number; gross_cents: number; discount_cents: number; vat_cents: number; net_cents: number; total_cents: number
  first_number: string | null; last_number: string | null; invoices: number; voids: number; void_total_cents: number
  by_method: Record<string, number>; top_items: { name: string; qty: number; total_cents: number }[]
}
const tz = import.meta.client ? Intl.DateTimeFormat().resolvedOptions().timeZone : 'Asia/Bangkok'
const { data: r } = await useAsyncData(`z:${route.query.shop}:${route.query.date}`, () =>
  api<Summary>(`/shops/${route.query.shop}/pos/summary`, { query: { date: route.query.date || undefined, tz } }),
)
const { t } = useI18n()
useAutoPrint(computed(() => !!r.value))
useHead({ title: () => t('pos.print.zReport'), style: [{ innerHTML: () => `@page { size: ${width.value}mm auto; margin: 0 }` }] })
const m = (c: number) => money(c, r.value?.shop.currency)
const labels: Record<string, string> = { cash: 'Cash', card: 'Card', transfer: 'Transfer', qr: 'QR / PromptPay', other: 'Other' }
/** Receipt paper → one language: Lao for Lao documents, otherwise English. */
const lo = computed(() => docLang(r.value?.shop.currency) === 'lo')
const tx = (key: DocKey, fallback: string) => (lo.value ? dlLocal(key, 'lo') : fallback)
const payLabel = (k: string) => (lo.value ? dlLocal(`pay_${k}`, 'lo') : labels[k] ?? k)
const dloc = computed(() => docLocale(lo.value ? 'lo' : null))
</script>

<template>
  <div v-if="r" class="zr mx-auto bg-white" :style="{ width: `${width}mm`, fontSize: width === 58 ? '10.5px' : '12px' }">
    <div class="text-center">
      <div class="text-[1.3em] font-extrabold">{{ r.shop.name }}</div>
      <div v-if="r.shop.legal_name">{{ r.shop.legal_name }}</div>
      <div v-if="r.shop.tax_id">{{ tx('taxId', 'TAX ID') }} {{ r.shop.tax_id }} {{ r.shop.branch }}</div>
      <div class="rule" />
      <div class="font-bold">{{ tx('zReport', 'END OF DAY REPORT (Z)') }}</div>
      <div>{{ fmtDate(r.date, 'full', dloc) }}</div>
    </div>
    <div class="rule" />
    <div class="row"><span>{{ tx('zSales', 'Sales') }}</span><span>{{ r.sales }}</span></div>
    <div class="row"><span>{{ tx('zReceipts', 'Receipts') }}</span><span>{{ r.first_number ?? '—' }} – {{ r.last_number ?? '—' }}</span></div>
    <div class="row"><span>{{ tx('zTaxInvoices', 'Tax invoices') }}</span><span>{{ r.invoices }}</span></div>
    <div class="rule" />
    <div class="row"><span>{{ tx('zGross', 'Gross sales') }}</span><span>{{ m(r.gross_cents) }}</span></div>
    <div class="row"><span>{{ tx('zBillDiscounts', 'Bill discounts') }}</span><span>−{{ m(r.discount_cents) }}</span></div>
    <div class="row"><span>{{ tx('beforeVatShort', 'Before VAT') }}</span><span>{{ m(r.net_cents) }}</span></div>
    <div class="row"><span>{{ tx('vatShort', 'VAT') }}</span><span>{{ m(r.vat_cents) }}</span></div>
    <div class="row text-[1.25em] font-extrabold"><span>{{ tx('total', 'TOTAL') }}</span><span>{{ m(r.total_cents) }}</span></div>
    <div class="rule" />
    <div class="font-bold">{{ tx('zByMethod', 'Takings by method') }}</div>
    <div v-for="(v, k) in r.by_method" :key="k" class="row"><span>{{ payLabel(k) }}</span><span>{{ m(v) }}</span></div>
    <div class="rule" />
    <div class="row"><span>{{ tx('zVoided', 'Voided sales') }}</span><span>{{ r.voids }} · {{ m(r.void_total_cents) }}</span></div>
    <template v-if="r.top_items.length">
      <div class="rule" />
      <div class="font-bold">{{ tx('zTopItems', 'Top items') }}</div>
      <div v-for="t in r.top_items" :key="t.name" class="row"><span class="truncate pr-2">{{ t.qty }}× {{ t.name }}</span><span>{{ m(t.total_cents) }}</span></div>
    </template>
    <div class="rule" />
    <div class="text-center opacity-70">{{ tx('printed', 'Printed') }} {{ fmtDateTime(r.generated_at, 'short', 'medium', dloc) }}</div>
    <div class="mt-6 text-center">{{ tx('signature', 'Signature') }} ______________________</div>
    <div class="no-print mt-6 flex justify-center pb-4 font-sans"><button class="btn-dark btn-sm" onclick="window.print()">{{ $t('common.print') }}</button></div>
  </div>
</template>

<style scoped>
.zr { padding: 4mm 3mm; font-family: "Noto Sans Thai", "Noto Sans Lao", "Plus Jakarta Sans", monospace; line-height: 1.4; color: #000; }
.rule { border-top: 1px dashed #000; margin: 5px 0; }
.row { display: flex; justify-content: space-between; font-variant-numeric: tabular-nums; }
@media screen { .zr { box-shadow: 0 4px 20px rgb(0 0 0 / .15); } }
</style>
