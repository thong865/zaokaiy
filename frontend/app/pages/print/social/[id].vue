<script setup lang="ts">
import type { SocialOrderDoc } from '~/utils/types'

/** Invoice or packing slip for a social (comment/chat) order. ?type=packing */
definePageMeta({ layout: 'print', middleware: 'auth' })
const route = useRoute()
const api = useApi()
const { data: doc } = await useAsyncData(`social-print:${route.params.id}`, () => api<SocialOrderDoc>(`/social/orders/${route.params.id}`))
const { t } = useI18n()
const packing = computed(() => route.query.type === 'packing')
const lang = computed(() => docLang(doc.value?.order.currency))
const loOnly = computed(() => (lang.value === 'lo' ? 'lo' : null))
useAutoPrint(computed(() => !!doc.value))
useHead({ title: () => `${packing.value ? t('pos.print.packingSlip') : t('pos.print.invoice')} ${doc.value?.order.number ?? ''}`, style: [{ innerHTML: '@page { size: A4; margin: 12mm }' }] })
const lines = computed(() => {
  const items = (doc.value?.items ?? []).map((i) => ({ name: i.name, sku: i.code, qty: i.qty, unit_price_cents: i.unit_price_cents, discount_cents: 0, line_total_cents: i.qty * i.unit_price_cents }))
  if (doc.value?.order.shipping_cents) items.push({ name: dl('shipping', lang.value), sku: '', qty: 1, unit_price_cents: doc.value.order.shipping_cents, discount_cents: 0, line_total_cents: doc.value.order.shipping_cents })
  return items
})
</script>

<template>
  <div v-if="doc">
    <div class="no-print mx-auto mb-4 flex w-[210mm] gap-2 font-sans">
      <button class="btn-dark btn-sm" onclick="window.print()">{{ $t('pos.print.printPdf') }}</button>
      <NuxtLink :to="{ query: { type: packing ? undefined : 'packing' } }" class="btn-ghost btn-sm">{{ packing ? $t('pos.print.showInvoice') : $t('pos.print.showPacking') }}</NuxtLink>
    </div>
    <InvoiceDocument
      :shop="doc.shop"
      :title="dlEn(packing ? 'packingSlip' : 'invoiceReceipt')"
      :title-local="lang ? dlLocal(packing ? 'packingSlip' : 'invoiceReceipt', lang) : ''"
      :number="doc.order.number"
      :date="doc.order.created_at"
      :ref-label="dl('channel', lang)"
      :ref-value="`${doc.customer.provider} · ${doc.customer.name}`"
      :buyer="{ name: doc.order.ship_name || doc.customer.name, address: doc.order.ship_address, phone: doc.order.ship_phone }"
      :lines="lines"
      :subtotal="doc.order.total_cents"
      :discount="0"
      :vat-bps="0"
      :vat="0"
      :before-vat="doc.order.total_cents"
      :total="doc.order.total_cents"
      :currency="doc.order.currency"
      :show-prices="!packing"
      :voided="['cancelled', 'expired'].includes(doc.order.status)"
      :note="doc.order.customer_note ? `${dlLocal('customerNote', loOnly)}: ${doc.order.customer_note}` : doc.order.tracking_no ? `${dlLocal('tracking', loOnly)}: ${doc.order.tracking_no}` : ''"
    />
  </div>
</template>
