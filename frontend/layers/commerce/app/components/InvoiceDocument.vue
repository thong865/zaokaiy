<script setup lang="ts">
import type { DocShop } from '~/utils/types'
import type { DocKey } from '#commerce/utils/docLabels'

/** A4 invoice / tax invoice layout shared by POS sales and online orders. */
interface InvoiceLine { name: string; sku?: string; qty: number; unit_price_cents: number; discount_cents: number; line_total_cents: number }
const props = withDefaults(defineProps<{
  shop: DocShop
  title: string
  titleLocal?: string
  number: string
  date: string
  refLabel?: string
  refValue?: string
  buyer: { name: string; tax_id?: string; branch?: string; address?: string; phone?: string; email?: string }
  lines: InvoiceLine[]
  subtotal: number
  discount: number
  vatBps: number
  vat: number
  beforeVat: number
  total: number
  currency: string
  copy?: boolean
  voided?: boolean
  note?: string
  showPrices?: boolean
}>(), { showPrices: true, copy: false, voided: false, titleLocal: '', refLabel: '', refValue: '', note: '' })
const m = (c: number) => money(c, props.currency)
const lang = computed(() => docLang(props.currency))
const L = (key: DocKey) => dl(key, lang.value)
const d = computed(() => fmtDocDate(props.date, docLocale(lang.value)))
const vatLabel = computed(() => {
  const local = lang.value ? dlLocal('vat', lang.value) : ''
  const en = `${dlEn('vat')} ${props.vatBps / 100}%`
  return local && local !== dlEn('vat') ? `${en} / ${local}` : en
})
</script>

<template>
  <article class="invoice relative mx-auto bg-white">
    <div v-if="voided" class="pointer-events-none absolute inset-0 grid place-items-center">
      <span class="-rotate-12 text-[120px] font-extrabold text-black/10">{{ dlLocal('void', lang) }}</span>
    </div>
    <header class="flex items-start justify-between gap-8 border-b-2 border-black pb-5">
      <div class="flex gap-4">
        <img v-if="shop.logo_url" :src="shop.logo_url" alt="" class="size-16 object-contain" >
        <div class="text-[12px] leading-relaxed">
          <div class="text-xl font-extrabold">{{ shop.legal_name || shop.name }}</div>
          <div v-if="shop.legal_name && shop.legal_name !== shop.name" class="font-semibold">{{ shop.name }}</div>
          <div v-if="shop.address" class="whitespace-pre-line">{{ shop.address }}</div>
          <div v-if="shop.phone">{{ L('tel') }} {{ shop.phone }}</div>
          <div v-if="shop.tax_id">{{ L('taxId') }} <b>{{ shop.tax_id }}</b><template v-if="shop.branch"> · {{ shop.branch }}</template></div>
        </div>
      </div>
      <div class="shrink-0 text-right">
        <div class="text-2xl font-extrabold leading-tight">{{ titleLocal || title }}</div>
        <div v-if="titleLocal" class="text-lg font-bold tracking-wide">{{ title }}</div>
        <div v-if="showPrices" class="mt-1 inline-block rounded border border-black px-2 py-0.5 text-[11px] font-bold">{{ copy ? dlr('copy', lang) : dlr('original', lang) }}</div>
        <table class="ml-auto mt-3 text-[12px]">
          <tbody>
            <tr><td class="pr-3 text-left text-gray-600">{{ L('no') }}</td><td class="font-bold">{{ number }}</td></tr>
            <tr><td class="pr-3 text-left text-gray-600">{{ L('date') }}</td><td class="font-bold">{{ d }}</td></tr>
            <tr v-if="refValue"><td class="pr-3 text-left text-gray-600">{{ refLabel }}</td><td>{{ refValue }}</td></tr>
          </tbody>
        </table>
      </div>
    </header>

    <section class="mt-5 rounded-lg border border-gray-400 p-4 text-[12px] leading-relaxed">
      <div class="text-[11px] font-bold uppercase text-gray-600">{{ L('customer') }}</div>
      <div class="text-[14px] font-bold">{{ buyer.name || '—' }}</div>
      <div v-if="buyer.address" class="whitespace-pre-line">{{ buyer.address }}</div>
      <div v-if="buyer.tax_id">{{ L('buyerTaxId') }} {{ buyer.tax_id }}<template v-if="buyer.branch"> · {{ buyer.branch }}</template></div>
      <div v-if="buyer.phone || buyer.email">{{ [buyer.phone, buyer.email].filter(Boolean).join(' · ') }}</div>
    </section>

    <table class="lines mt-5 w-full text-[12px]">
      <thead>
        <tr>
          <th class="w-10">#</th>
          <th class="text-left">{{ L('description') }}</th>
          <th class="w-16">{{ L('qty') }}</th>
          <template v-if="showPrices">
            <th class="w-28 text-right">{{ L('unitPrice') }}</th>
            <th class="w-24 text-right">{{ L('colDiscount') }}</th>
            <th class="w-32 text-right">{{ L('amount') }}</th>
          </template>
          <th v-else class="w-24">✓</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(l, i) in lines" :key="i">
          <td class="text-center">{{ i + 1 }}</td>
          <td>{{ l.name }}<span v-if="l.sku" class="text-gray-500"> · {{ l.sku }}</span></td>
          <td class="text-center tabular-nums">{{ l.qty }}</td>
          <template v-if="showPrices">
            <td class="text-right tabular-nums">{{ m(l.unit_price_cents) }}</td>
            <td class="text-right tabular-nums">{{ l.discount_cents ? m(l.discount_cents) : '' }}</td>
            <td class="text-right tabular-nums">{{ m(l.line_total_cents) }}</td>
          </template>
          <td v-else><span class="inline-block size-4 border border-black" /></td>
        </tr>
      </tbody>
    </table>

    <div v-if="showPrices" class="mt-4 flex items-start justify-between gap-8">
      <div class="max-w-[55%] text-[12px]">
        <div class="rounded-lg bg-gray-100 p-3">
          <div v-if="currency === 'THB'" class="font-bold">({{ bahtText(total) }})</div>
          <div v-else-if="currency === 'LAK'" class="font-bold">({{ kipText(total) }})</div>
          <div class="italic">{{ amountInEnglish(total, currency) }}</div>
        </div>
        <p v-if="note" class="mt-3 whitespace-pre-line text-gray-600">{{ note }}</p>
      </div>
      <table class="totals min-w-72 text-[12px]">
        <tbody>
          <tr><td>{{ L('subtotal') }}</td><td>{{ m(subtotal) }}</td></tr>
          <tr v-if="discount"><td>{{ L('discount') }}</td><td>−{{ m(discount) }}</td></tr>
          <template v-if="vatBps">
            <tr><td>{{ L('beforeVat') }}</td><td>{{ m(beforeVat) }}</td></tr>
            <tr><td>{{ vatLabel }}</td><td>{{ m(vat) }}</td></tr>
          </template>
          <tr class="grand"><td>{{ L('grandTotal') }}</td><td>{{ m(total) }}</td></tr>
        </tbody>
      </table>
    </div>

    <footer class="mt-16 grid grid-cols-2 gap-16 text-center text-[12px]">
      <div><div class="border-t border-black pt-2">{{ L('receivedBy') }}</div><div class="mt-1 text-gray-500">{{ dlLocal('date', lang === 'lo' ? 'lo' : null) }} ____/____/______</div></div>
      <div><div class="border-t border-black pt-2">{{ L('authorizedSignature') }}</div><div class="mt-1 text-gray-500">{{ shop.legal_name || shop.name }}</div></div>
    </footer>
  </article>
</template>

<style scoped>
.invoice { width: 210mm; min-height: 297mm; padding: 14mm; font-family: "Noto Sans Thai", "Noto Sans Lao", "Plus Jakarta Sans", sans-serif; color: #000; }
.lines th { border-top: 1.5px solid #000; border-bottom: 1.5px solid #000; padding: 6px 6px; font-size: 11px; }
.lines td { border-bottom: 1px solid #ddd; padding: 6px 6px; vertical-align: top; }
.totals td { padding: 4px 0 4px 12px; }
.totals td:last-child { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
.totals .grand td { border-top: 1.5px solid #000; border-bottom: 3px double #000; font-weight: 800; font-size: 14px; padding-top: 6px; padding-bottom: 6px; }
@media screen { .invoice { box-shadow: 0 4px 24px rgb(0 0 0 / .15); } }
@media print { .invoice { width: auto; min-height: auto; padding: 0; box-shadow: none; } }
</style>
