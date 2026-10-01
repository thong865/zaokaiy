<script setup lang="ts">
import type { PosDoc } from '~/utils/types'
import type { DocKey } from '#commerce/utils/docLabels'

/**
 * Thermal receipt (80 mm or 58 mm), shared by the print page and the POS bill view.
 * Doubles as an abbreviated tax invoice when the shop has a tax ID.
 * `preview` = a bill that isn't paid yet (POS bill view): marked as such.
 */
const props = withDefaults(defineProps<{ doc: PosDoc; width?: 58 | 80; preview?: boolean }>(), { width: 80, preview: false })
const s = computed(() => props.doc.sale)
const cur = computed(() => s.value.currency)
const m = (c: number) => money(c, cur.value)
const hasTax = computed(() => !!props.doc.shop.tax_id)
const methodLabel: Record<string, string> = { cash: 'Cash / เงินสด', card: 'Card / บัตร', transfer: 'Transfer / โอน', qr: 'QR / PromptPay', other: 'Other' }
/** Narrow paper → one language: Lao for Lao documents, otherwise the original English/Thai wording. */
const lo = computed(() => docLang(cur.value) === 'lo')
const tx = (key: DocKey | (string & {}), fallback: string) => (lo.value ? dlLocal(key, 'lo') : fallback)
const payLabel = (m: string) => (lo.value ? dlLocal(`pay_${m}`, 'lo') : methodLabel[m] ?? m)
const when = computed(() => fmtDateTime(s.value.created_at, 'short', 'short', docLocale(lo.value ? 'lo' : null)))
const width = computed(() => props.width)
const doc = computed(() => props.doc)
</script>

<template>
  <div class="receipt relative mx-auto bg-white" :style="{ width: `${width}mm`, fontSize: width === 58 ? '10.5px' : '12px' }">
    <div v-if="preview" class="mb-1 rounded border border-dashed border-black/60 px-1 text-center text-[0.85em] font-bold">{{ tx('preview', 'PREVIEW — NOT PAID YET') }}</div>
    <div v-if="s.status === 'voided'" class="pointer-events-none absolute inset-x-0 top-1/3 -rotate-12 text-center text-5xl font-extrabold text-black/15">{{ tx('void', 'VOID') }}</div>
    <div class="text-center">
      <img v-if="doc.shop.logo_url" :src="doc.shop.logo_url" alt="" class="mx-auto mb-1 max-h-12 object-contain grayscale" >
      <div class="text-[1.35em] font-extrabold leading-tight">{{ doc.shop.name }}</div>
      <div v-if="doc.shop.legal_name">{{ doc.shop.legal_name }}</div>
      <div v-if="doc.shop.address" class="whitespace-pre-line">{{ doc.shop.address }}</div>
      <div v-if="doc.shop.phone">{{ tx('tel', 'Tel.') }} {{ doc.shop.phone }}</div>
      <div v-if="hasTax">{{ tx('taxId', 'TAX ID') }} {{ doc.shop.tax_id }}<template v-if="doc.shop.branch"> · {{ doc.shop.branch }}</template></div>
    </div>

    <div class="rule" />
    <div class="text-center font-bold">
      <template v-if="lo">{{ dlLocal(hasTax ? 'receiptAbb' : 'receipt', 'lo') }}</template>
      <template v-else-if="hasTax">ใบเสร็จรับเงิน/ใบกำกับภาษีอย่างย่อ<br>RECEIPT / TAX INVOICE (ABB)</template>
      <template v-else>ใบเสร็จรับเงิน / RECEIPT</template>
    </div>
    <div class="mt-1 flex justify-between"><span>{{ tx('no', 'No.') }} {{ s.number }}</span><span>{{ when }}</span></div>
    <div v-if="doc.cashier">{{ tx('cashier', 'Cashier') }}: {{ doc.cashier }}</div>
    <div class="rule" />

    <div v-for="it in doc.items" :key="it.id" class="mb-1">
      <div class="break-words">{{ it.name }}</div>
      <div class="flex justify-between tabular-nums">
        <span class="pl-2">{{ it.qty }} × {{ m(it.unit_price_cents) }}</span>
        <span>{{ m(it.qty * it.unit_price_cents) }}</span>
      </div>
      <div v-if="it.discount_cents" class="flex justify-between pl-2 tabular-nums"><span>{{ tx('discount', 'Discount') }}</span><span>−{{ m(it.discount_cents) }}</span></div>
    </div>
    <div class="rule" />

    <div class="flex justify-between"><span>{{ tx('items', 'Items') }}</span><span>{{ doc.items.reduce((n, i) => n + i.qty, 0) }}</span></div>
    <div class="flex justify-between tabular-nums"><span>{{ tx('subtotal', 'Subtotal') }}</span><span>{{ m(s.subtotal_cents) }}</span></div>
    <div v-if="s.discount_cents" class="flex justify-between tabular-nums"><span>{{ tx('discount', 'Discount') }}</span><span>−{{ m(s.discount_cents) }}</span></div>
    <div class="my-1 flex justify-between text-[1.3em] font-extrabold tabular-nums"><span>{{ tx('total', 'TOTAL') }}</span><span>{{ m(s.total_cents) }}</span></div>
    <template v-if="s.vat_bps">
      <div class="flex justify-between tabular-nums"><span>{{ tx('beforeVatShort', 'Before VAT') }}</span><span>{{ m(doc.amount_before_vat_cents) }}</span></div>
      <div class="flex justify-between tabular-nums"><span>{{ tx('vatShort', 'VAT') }} {{ s.vat_bps / 100 }}%{{ s.prices_include_vat ? ` ${tx('vatIncl', '(incl.)')}` : '' }}</span><span>{{ m(s.vat_cents) }}</span></div>
    </template>
    <div class="rule" />
    <div v-for="(p, i) in s.payments" :key="i" class="flex justify-between tabular-nums">
      <span>{{ payLabel(p.method) }}<template v-if="p.reference"> ({{ p.reference }})</template></span><span>{{ m(p.amount_cents) }}</span>
    </div>
    <div v-if="s.change_cents" class="flex justify-between font-bold tabular-nums"><span>{{ tx('change', 'Change / เงินทอน') }}</span><span>{{ m(s.change_cents) }}</span></div>

    <template v-if="s.invoice_number || s.note || s.status === 'voided'">
      <div class="rule" />
      <div v-if="s.invoice_number">{{ tx('fullTaxInvoice', 'Full tax invoice') }}: {{ s.invoice_number }}</div>
      <div v-if="s.note" class="whitespace-pre-line">{{ s.note }}</div>
      <div v-if="s.status === 'voided'" class="font-bold">{{ tx('voided', 'VOIDED') }}: {{ s.void_reason }}</div>
    </template>
    <div class="rule" />
    <div class="whitespace-pre-line text-center">{{ doc.shop.receipt_footer }}</div>
    <div class="mt-1 text-center text-[0.85em] opacity-70">{{ s.number }} · {{ tx('vatIncluded', 'VAT included') }} · zaokaiy POS</div>
  </div>
</template>

<style scoped>
.receipt { padding: 4mm 3mm; font-family: "Noto Sans Thai", "Noto Sans Lao", "Plus Jakarta Sans", ui-monospace, monospace; line-height: 1.35; color: #000; }
.rule { border-top: 1px dashed #000; margin: 5px 0; }
</style>
