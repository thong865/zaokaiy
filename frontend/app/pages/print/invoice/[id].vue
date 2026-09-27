<script setup lang="ts">
import type { PosDoc } from '~/utils/types'

/** Full tax invoice (A4) for a POS sale. ?copy=1 prints the "COPY" version. */
definePageMeta({ layout: 'print', middleware: 'auth' })
const route = useRoute()
const api = useApi()
const { data: doc } = await useAsyncData(`invoice:${route.params.id}`, () => api<PosDoc>(`/pos/sales/${route.params.id}`))
const { t } = useI18n()
const copy = computed(() => route.query.copy === '1')
const lang = computed(() => docLang(doc.value?.sale.currency))
useAutoPrint(computed(() => !!doc.value))
useHead({
  title: () => (doc.value?.sale.invoice_number ? t('pos.print.taxInvoiceNo', { number: doc.value.sale.invoice_number }) : t('pos.print.taxInvoice')),
  style: [{ innerHTML: '@page { size: A4; margin: 12mm }' }],
})
</script>

<template>
  <div v-if="doc">
    <div class="no-print mx-auto mb-4 flex w-[210mm] items-center gap-2 font-sans">
      <button class="btn-dark btn-sm" onclick="window.print()">{{ $t('pos.print.printPdf') }}</button>
      <NuxtLink :to="{ query: { copy: copy ? undefined : '1' } }" class="btn-ghost btn-sm">{{ copy ? $t('pos.print.showOriginal') : $t('pos.print.showCopy') }}</NuxtLink>
      <span v-if="!doc.sale.invoice_number" class="text-sm text-brand-700">{{ $t('pos.print.noInvoice') }}</span>
    </div>
    <InvoiceDocument
      v-if="doc.sale.invoice_number"
      :shop="doc.shop"
      :title="dlEn('taxInvoice')"
      :title-local="lang ? dlLocal('taxInvoice', lang) : ''"
      :number="doc.sale.invoice_number"
      :date="doc.sale.invoice_issued_at ?? doc.sale.created_at"
      :ref-label="dl('receiptRef', lang)"
      :ref-value="doc.sale.number"
      :buyer="{ name: doc.sale.customer_name, tax_id: doc.sale.customer_tax_id, branch: doc.sale.customer_branch, address: doc.sale.customer_address, phone: doc.sale.customer_phone }"
      :lines="doc.items"
      :subtotal="doc.sale.subtotal_cents"
      :discount="doc.sale.discount_cents"
      :vat-bps="doc.sale.vat_bps"
      :vat="doc.sale.vat_cents"
      :before-vat="doc.amount_before_vat_cents"
      :total="doc.sale.total_cents"
      :currency="doc.sale.currency"
      :copy="copy"
      :voided="doc.sale.status === 'voided'"
      :note="doc.sale.status === 'voided' ? `${dlLocal('voided', lang === 'lo' ? 'lo' : null)} — ${doc.sale.void_reason}` : doc.sale.note"
    />
  </div>
</template>
