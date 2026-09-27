<script setup lang="ts">
import type { DocShop, Order } from '~/utils/types'

/** Online order documents: ?type=invoice (default) or ?type=packing (packing slip, no prices). */
definePageMeta({ layout: 'print', middleware: 'auth' })
const route = useRoute()
const api = useApi()
interface OrderDoc { order: Order; items: Order['items']; buyer: { name: string; email: string }; vat_bps: number; vat_cents: number; shop: DocShop }
const { data: doc } = await useAsyncData(`order-doc:${route.params.id}`, () => api<OrderDoc>(`/orders/${route.params.id}/document`))
const { t, te } = useI18n()
const packing = computed(() => route.query.type === 'packing')
const lang = computed(() => docLang(doc.value?.order.currency))
const titleKey = computed(() => (packing.value ? 'packingSlip' : doc.value?.shop.tax_id ? 'invoiceTaxInvoice' : 'invoiceReceipt'))
const loOnly = computed(() => (lang.value === 'lo' ? 'lo' : null))
useAutoPrint(computed(() => !!doc.value))
useHead({
  title: () => `${packing.value ? t('pos.print.packingSlip') : t('pos.print.invoice')} #${String(route.params.id).slice(0, 8).toUpperCase()}`,
  style: [{ innerHTML: '@page { size: A4; margin: 12mm }' }],
})
const addr = computed(() => {
  const a = doc.value?.order.shipping_address ?? {}
  return [a.line1, [a.city, a.postcode].filter(Boolean).join(' ')].filter(Boolean).join('\n')
})
const lines = computed(() =>
  (doc.value?.items ?? []).map((i) => ({ name: i.product_name, qty: i.qty, unit_price_cents: i.unit_price_cents, discount_cents: 0, line_total_cents: i.unit_price_cents * i.qty })),
)
</script>

<template>
  <div v-if="doc">
    <div class="no-print mx-auto mb-4 flex w-[210mm] gap-2 font-sans">
      <button class="btn-dark btn-sm" onclick="window.print()">{{ $t('pos.print.printPdf') }}</button>
      <NuxtLink :to="{ query: { type: packing ? 'invoice' : 'packing' } }" class="btn-ghost btn-sm">{{ packing ? $t('pos.print.showInvoice') : $t('pos.print.showPacking') }}</NuxtLink>
    </div>
    <InvoiceDocument
      :shop="doc.shop"
      :title="dlEn(titleKey)"
      :title-local="lang ? dlLocal(titleKey, lang) : ''"
      :number="`ORD-${doc.order.id.slice(0, 8).toUpperCase()}`"
      :date="doc.order.created_at"
      :ref-label="dl('status', lang)"
      :ref-value="te(`common.status.${doc.order.status}`) ? t(`common.status.${doc.order.status}`) : doc.order.status"
      :buyer="{ name: doc.order.shipping_address?.name || doc.buyer.name, address: addr, phone: doc.order.shipping_address?.phone, email: doc.buyer.email }"
      :lines="lines"
      :subtotal="doc.order.total_cents"
      :discount="0"
      :vat-bps="doc.vat_bps"
      :vat="doc.vat_cents"
      :before-vat="doc.order.total_cents - doc.vat_cents"
      :total="doc.order.total_cents"
      :currency="doc.order.currency"
      :show-prices="!packing"
      :voided="doc.order.status === 'cancelled'"
      :note="doc.order.status === 'pending' ? dlLocal('awaitingPayment', loOnly) : doc.shop.receipt_footer"
    />
  </div>
</template>
