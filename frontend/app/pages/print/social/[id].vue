<script setup lang="ts">
import type { SocialOrderDoc } from '~/utils/types'

/** Invoice or packing slip for a social (comment/chat) order. ?type=packing */
definePageMeta({ colorMode: 'light', layout: 'print', middleware: 'auth' })
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
  if (doc.value?.order.cod_fee_cents) items.push({ name: dl('codFee', lang.value), sku: '', qty: 1, unit_price_cents: doc.value.order.cod_fee_cents, discount_cents: 0, line_total_cents: doc.value.order.cod_fee_cents })
  return items
})
const { byCode } = useCarriers()
const note = computed(() => {
  const o = doc.value?.order
  if (!o) return ''
  const c = byCode(o.carrier_code)
  const courier = c ? (loOnly.value && c.name_lo ? `${c.name_lo} (${c.name})` : c.name) : o.carrier_code
  return [
    o.customer_note ? `${dlLocal('customerNote', loOnly.value)}: ${o.customer_note}` : '',
    o.payment_method === 'cod' ? `${dlLocal('cod', loOnly.value)}: ${money(o.cod_amount_cents, o.currency)}` : '',
    courier ? `${dlLocal('courier', loOnly.value)}: ${courier}${o.tracking_no ? ` · ${dlLocal('tracking', loOnly.value)}: ${o.tracking_no}` : ''}` : o.tracking_no ? `${dlLocal('tracking', loOnly.value)}: ${o.tracking_no}` : '',
    o.carrier_code && o.fee_payer === 'destination' ? dlLocal('shippingAtDestination', loOnly.value) : '',
  ].filter(Boolean).join('\n')
})
</script>

<template>
  <div v-if="doc">
    <div class="no-print mx-auto mb-4 flex w-[210mm] gap-2 font-sans">
      <UButton color="neutral" size="sm" type="submit" onclick="window.print()">{{ $t('pos.print.printPdf') }}</UButton>
      <UButton color="neutral" variant="soft" size="sm" :to="{ query: { type: packing ? undefined : 'packing' } }">{{ packing ? $t('pos.print.showInvoice') : $t('pos.print.showPacking') }}</UButton>
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
      :note="note"
    />
  </div>
</template>
