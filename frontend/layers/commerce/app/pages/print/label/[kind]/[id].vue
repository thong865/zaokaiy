<script setup lang="ts">
import type { DocShop, FeePayer, Order, SocialOrderDoc } from '~/utils/types'

/** A6 (100 × 150 mm) courier shipping label for a marketplace order (`order`) or a comment/chat order (`social`). */
definePageMeta({ colorMode: 'light', layout: 'print', middleware: 'auth' })
const route = useRoute()
const api = useApi()
const { t } = useI18n()
const { byCode } = useCarriers()
const kind = computed(() => (route.params.kind === 'social' ? 'social' : 'order'))
interface OrderDoc { order: Order; items: Order['items']; buyer: { name: string; email: string }; shop: DocShop }

interface Label {
  number: string
  date: string
  carrier: string | null
  deliveryType: 'branch' | 'home'
  feePayer: FeePayer
  from: { name: string; phone: string; address: string }
  to: { name: string; phone: string; address: string; branch: string }
  tracking: string
  items: { name: string; qty: number }[]
  cod: boolean
  codAmount: number
  currency: string
  cancelled: boolean
}

const { data: label } = await useAsyncData(`label:${route.params.kind}:${route.params.id}`, async (): Promise<Label> => {
  if (kind.value === 'social') {
    const d = await api<SocialOrderDoc>(`/social/orders/${route.params.id}`)
    const o = d.order
    return {
      number: o.number,
      date: o.created_at,
      carrier: o.carrier_code,
      deliveryType: o.delivery_type,
      feePayer: o.fee_payer,
      from: { name: d.shop.name, phone: d.shop.phone, address: d.shop.address },
      to: { name: o.ship_name || d.customer.name, phone: o.ship_phone, address: o.ship_address, branch: '' },
      tracking: o.tracking_no,
      items: d.items.map((i) => ({ name: i.code ? `${i.code} ${i.name}` : i.name, qty: i.qty })),
      cod: o.payment_method === 'cod',
      codAmount: o.cod_amount_cents,
      currency: o.currency,
      cancelled: ['cancelled', 'expired'].includes(o.status),
    }
  }
  const d = await api<OrderDoc>(`/orders/${route.params.id}/document`)
  const o = d.order
  const a = o.shipping_address ?? {}
  return {
    number: `ORD-${o.id.slice(0, 8).toUpperCase()}`,
    date: o.created_at,
    carrier: o.carrier_code,
    deliveryType: o.delivery_type,
    feePayer: o.fee_payer,
    from: { name: d.shop.name, phone: d.shop.phone, address: d.shop.address },
    to: {
      name: a.name || d.buyer.name,
      phone: a.phone || '',
      address: a.address || [a.line1, [a.city, a.postcode].filter(Boolean).join(' ')].filter(Boolean).join('\n'),
      branch: a.branch || '',
    },
    tracking: o.tracking_no,
    items: d.items.map((i) => ({ name: i.product_name, qty: i.qty })),
    cod: o.payment_method === 'cod',
    codAmount: o.cod_amount_cents,
    currency: o.currency,
    cancelled: o.status === 'cancelled',
  }
})

// A label is read by courier staff: static wording is always Lao + English, whatever the UI language.
const L = {
  from: 'ຜູ້ສົ່ງ / FROM',
  to: 'ຜູ້ຮັບ / TO',
  tracking: 'ເລກພັດສະດຸ / TRACKING NO.',
  order: 'ຄຳສັ່ງຊື້ / ORDER',
  items: 'ສິນຄ້າ / ITEMS',
  pcs: 'ອັນ / pcs',
  branch: 'ຮັບທີ່ສາຂາ / BRANCH PICKUP',
  home: 'ສົ່ງເຖິງບ້ານ / HOME DELIVERY',
  cod: 'ເກັບເງິນປາຍທາງ',
  paid: 'ຈ່າຍແລ້ວ / PAID',
  feeDestination: 'ປາຍທາງຈ່າຍຄ່າສົ່ງ / Shipping fee paid by receiver',
  tel: 'ໂທ / Tel.',
  cancelled: 'ຍົກເລີກ / CANCELLED',
}
const carrier = computed(() => byCode(label.value?.carrier))
const qtyTotal = computed(() => (label.value?.items ?? []).reduce((s, i) => s + i.qty, 0))
const shortItems = computed(() => {
  const list = label.value?.items ?? []
  return { shown: list.slice(0, 4), more: Math.max(0, list.length - 4) }
})

useAutoPrint(computed(() => !!label.value))
useHead({
  title: () => `${t('ship.label.title')} ${label.value?.number ?? ''}`,
  style: [{ innerHTML: '@page { size: 100mm 150mm; margin: 4mm }' }],
})
</script>

<template>
  <div v-if="label">
    <div class="no-print mx-auto mb-4 flex w-[100mm] gap-2 font-sans">
      <UButton color="neutral" size="sm" type="submit" onclick="window.print()">{{ $t('common.print') }}</UButton>
      <UButton color="neutral" variant="soft" size="sm" type="submit" onclick="history.length > 1 ? history.back() : window.close()">{{ $t('common.back') }}</UButton>
    </div>

    <article class="label relative mx-auto bg-white">
      <div v-if="label.cancelled" class="pointer-events-none absolute inset-0 grid place-items-center">
        <span class="-rotate-12 text-[34px] font-extrabold text-black/15">{{ L.cancelled }}</span>
      </div>

      <header class="flex items-end justify-between gap-2 border-b-[3px] border-black pb-1.5">
        <div class="min-w-0">
          <div class="truncate text-[20px] font-extrabold leading-tight">{{ carrier?.name_lo || carrier?.name || label.carrier || '—' }}</div>
          <div v-if="carrier && carrier.name_lo && carrier.name_lo !== carrier.name" class="truncate text-[13px] font-bold uppercase leading-tight">{{ carrier.name }}</div>
        </div>
        <div class="shrink-0 rounded border-2 border-black px-1.5 py-0.5 text-center text-[10px] font-extrabold leading-tight">
          {{ label.deliveryType === 'home' ? L.home : L.branch }}
        </div>
      </header>

      <section class="grid grid-cols-[1fr_auto] gap-2 border-b border-black py-1.5">
        <div class="min-w-0">
          <div class="cap">{{ L.tracking }}</div>
          <div v-if="label.tracking" class="break-all font-mono text-[20px] font-extrabold leading-tight tracking-wide">{{ label.tracking }}</div>
          <div v-else class="mt-4 border-b-2 border-dashed border-black" />
        </div>
        <div class="text-right">
          <div class="cap">{{ L.order }}</div>
          <div class="font-mono text-[12px] font-bold">{{ label.number }}</div>
          <div class="text-[9px]">{{ fmtDate(label.date, 'short', 'lo') }}</div>
        </div>
      </section>

      <section class="border-b border-black py-1.5">
        <div class="cap">{{ L.to }}</div>
        <div class="text-[16px] font-extrabold leading-tight">{{ label.to.name || '—' }}</div>
        <div class="text-[15px] font-bold">{{ L.tel }} {{ label.to.phone || '—' }}</div>
        <div class="whitespace-pre-line text-[11px] leading-snug">{{ label.to.address }}</div>
        <div v-if="label.deliveryType === 'branch' && label.to.branch" class="mt-0.5 text-[12px] font-bold">{{ L.branch }}: {{ label.to.branch }}</div>
      </section>

      <section class="border-b border-black py-1.5 text-[10px] leading-snug">
        <div class="cap">{{ L.from }}</div>
        <div class="text-[12px] font-bold">{{ label.from.name }}</div>
        <div v-if="label.from.phone">{{ L.tel }} {{ label.from.phone }}</div>
        <div v-if="label.from.address" class="line-clamp-2 whitespace-pre-line">{{ label.from.address }}</div>
      </section>

      <section class="border-b border-black py-1.5 text-[10px] leading-snug">
        <div class="cap">{{ L.items }} · {{ qtyTotal }} {{ L.pcs }}</div>
        <div v-for="(i, n) in shortItems.shown" :key="n" class="flex justify-between gap-2">
          <span class="truncate">{{ i.name }}</span><span class="shrink-0 font-bold">× {{ i.qty }}</span>
        </div>
        <div v-if="shortItems.more">+{{ shortItems.more }} …</div>
      </section>

      <section class="mt-2">
        <div v-if="label.cod" class="rounded-md border-[3px] border-black bg-black px-2 py-1.5 text-center text-white">
          <div class="text-[14px] font-extrabold leading-tight">{{ L.cod }} · COD</div>
          <div class="text-[26px] font-extrabold leading-tight tabular-nums">{{ money(label.codAmount, label.currency) }}</div>
        </div>
        <div v-else class="rounded-md border-[3px] border-black px-2 py-2 text-center text-[20px] font-extrabold">{{ L.paid }}</div>
        <div v-if="label.feePayer === 'destination'" class="mt-1.5 rounded border-2 border-black px-1.5 py-1 text-center text-[11px] font-bold">{{ L.feeDestination }}</div>
      </section>
    </article>
  </div>
</template>

<style scoped>
.label { width: 100mm; min-height: 150mm; padding: 4mm; font-family: "Noto Sans Lao", "Noto Sans Thai", "Plus Jakarta Sans", sans-serif; color: #000; }
.cap { font-size: 8.5px; font-weight: 700; text-transform: uppercase; letter-spacing: .03em; color: #333; }
@media screen { .label { box-shadow: 0 4px 24px rgb(0 0 0 / .15); } }
@media print {
  .label { width: auto; min-height: auto; padding: 0; box-shadow: none; }
  .bg-black { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
}
</style>
