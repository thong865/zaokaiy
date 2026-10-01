<script setup lang="ts">
import type { DocShop, Order } from '~/utils/types'

/** Online order documents: ?type=invoice (default) or ?type=packing (packing slip, no prices). */
definePageMeta({ colorMode: 'light', layout: 'print', middleware: 'auth' })
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
const { byCode } = useCarriers()
const lines = computed(() => {
  const o = doc.value?.order
  const out = (doc.value?.items ?? []).map((i) => ({ name: i.product_name, qty: i.qty, unit_price_cents: i.unit_price_cents, discount_cents: 0, line_total_cents: i.unit_price_cents * i.qty }))
  if (!o) return out
  const extra = (name: string, c: number) => out.push({ name, qty: 1, unit_price_cents: c, discount_cents: 0, line_total_cents: c })
  if (o.fee_payer === 'buyer' && o.shipping_fee_cents > 0) extra(dl('shipping', lang.value), o.shipping_fee_cents)
  if (o.cod_fee_cents > 0) extra(dl('codFee', lang.value), o.cod_fee_cents)
  return out
})
const grand = computed(() => doc.value?.order.grand_total_cents ?? doc.value?.order.total_cents ?? 0)
const note = computed(() => {
  const o = doc.value?.order
  if (!o) return ''
  const c = byCode(o.carrier_code)
  const courier = c ? (loOnly.value && c.name_lo ? `${c.name_lo} (${c.name})` : c.name) : o.carrier_code
  return [
    o.payment_method === 'cod' ? `${dlLocal('cod', loOnly.value)}: ${money(o.cod_amount_cents, o.currency)}` : o.status === 'pending' ? dlLocal('awaitingPayment', loOnly.value) : '',
    courier ? `${dlLocal('courier', loOnly.value)}: ${courier}${o.tracking_no ? ` · ${dlLocal('tracking', loOnly.value)}: ${o.tracking_no}` : ''}` : o.tracking_no ? `${dlLocal('tracking', loOnly.value)}: ${o.tracking_no}` : '',
    o.fee_payer === 'destination' ? dlLocal('shippingAtDestination', loOnly.value) : '',
    o.status !== 'pending' || o.payment_method === 'cod' ? doc.value?.shop.receipt_footer ?? '' : '',
  ].filter(Boolean).join('\n')
})
</script>

<template>
  <div v-if="doc">
    <div class="no-print mx-auto mb-4 flex w-[210mm] gap-2 font-sans">
      <UButton color="neutral" size="sm" type="submit" onclick="window.print()">{{ $t('pos.print.printPdf') }}</UButton>
      <UButton color="neutral" variant="soft" size="sm" :to="{ query: { type: packing ? 'invoice' : 'packing' } }">{{ packing ? $t('pos.print.showInvoice') : $t('pos.print.showPacking') }}</UButton>
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
      :subtotal="grand"
      :discount="0"
      :vat-bps="doc.vat_bps"
      :vat="doc.vat_cents"
      :before-vat="grand - doc.vat_cents"
      :total="grand"
      :currency="doc.order.currency"
      :show-prices="!packing"
      :voided="doc.order.status === 'cancelled'"
      :note="note"
    />
  </div>
</template>
