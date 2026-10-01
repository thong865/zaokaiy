<script setup lang="ts">
import type { PosDoc, PosPayment } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const { t, te } = useI18n()
const api = useApi()
const { shopId, shop, can } = useShop()
const { print, open } = usePrint()
const cur = computed(() => shop.value?.currency ?? 'THB')
const tz = import.meta.client ? Intl.DateTimeFormat().resolvedOptions().timeZone : 'Asia/Bangkok'
const today = () => new Date(Date.now() - new Date().getTimezoneOffset() * 60000).toISOString().slice(0, 10)
const date = ref(today())
const q = ref('')
const qd = ref('')
let timer: ReturnType<typeof setTimeout> | undefined
watch(q, (v) => { clearTimeout(timer); timer = setTimeout(() => (qd.value = v), 250) })

interface Row { id: string; number: string; status: string; total_cents: number; vat_cents: number; payments: PosPayment[]; invoice_number: string | null; customer_name: string; created_at: string; units: number; cashier: string | null }
interface Summary { sales: number; total_cents: number; vat_cents: number; discount_cents: number; voids: number; void_total_cents: number; invoices: number; by_method: Record<string, number> }
const { data: rows, refresh } = await useAsyncData(
  'pos-sales',
  () => api<Row[]>(`/shops/${shopId.value}/pos/sales`, { query: { date: qd.value ? undefined : date.value, q: qd.value || undefined, tz, limit: 300 } }),
  { watch: [shopId, date, qd], default: () => [] },
)
const { data: sum, refresh: refreshSum } = await useAsyncData(
  'pos-summary',
  () => api<Summary>(`/shops/${shopId.value}/pos/summary`, { query: { date: date.value, tz } }),
  { watch: [shopId, date] },
)
const methodLabel = (m: string) => (te(`common.pay.${m}`) ? t(`common.pay.${m}`) : m)
const paperWidth = ref<'80' | '58'>('80')
onMounted(() => { try { paperWidth.value = (localStorage.getItem('zk_paper') as '80' | '58') || '80' } catch {} })

const error = ref('')
const voiding = ref<Row | null>(null)
const voidReason = ref('')
async function doVoid() {
  if (!voiding.value) return
  error.value = ''
  try {
    await api(`/pos/sales/${voiding.value.id}/void`, { method: 'POST', body: { reason: voidReason.value } })
    voiding.value = null
    voidReason.value = ''
    await Promise.all([refresh(), refreshSum()])
  } catch (e) {
    error.value = apiError(e)
  }
}
const invoicing = ref<Row | null>(null)
const inv = reactive({ name: '', tax_id: '', branch: '', address: '', phone: '' })
async function doInvoice() {
  if (!invoicing.value) return
  error.value = ''
  try {
    const d = await api<PosDoc>(`/pos/sales/${invoicing.value.id}/invoice`, {
      method: 'POST',
      body: { customer_name: inv.name, customer_tax_id: inv.tax_id, customer_branch: inv.branch, customer_address: inv.address, customer_phone: inv.phone },
    })
    invoicing.value = null
    Object.assign(inv, { name: '', tax_id: '', branch: '', address: '', phone: '' })
    await refresh()
    print(`/print/invoice/${d.sale.id}`)
  } catch (e) {
    error.value = apiError(e)
  }
}
</script>

<template>
  <div>
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 class="page-title">{{ $t('common.dashNav.posSales') }}</h1>
        <p class="text-sm text-muted">{{ $t('pos.sales.subtitle') }}</p>
      </div>
      <div class="flex gap-2">
        <UButton to="/pos">{{ $t('pos.sales.openPos') }}</UButton>
      </div>
    </div>

    <div class="mt-6 flex flex-wrap items-center gap-3">
      <UInput v-model="date" type="date" class="w-44" :max="today()" :aria-label="$t('common.date')" />
      <UInput v-model="q" class="w-64" :placeholder="$t('pos.sales.searchPlaceholder')" />
      <div class="ml-auto flex items-center gap-2">
        <select v-model="paperWidth" class="input w-24 py-2" :aria-label="$t('pos.paperWidth')"><option value="80">80 mm</option><option value="58">58 mm</option></select>
        <UButton color="neutral" variant="soft" type="submit" @click="print(`/print/z-report?shop=${shopId}&date=${date}&w=${paperWidth}`)">{{ $t('pos.sales.zReport') }}</UButton>
      </div>
    </div>

    <div v-if="sum" class="mt-4 grid grid-cols-2 gap-4 lg:grid-cols-5">
      <StatCard :label="$t('pos.sales.takings')" :value="money(sum.total_cents, cur)" :hint="$t('pos.sales.salesCount', sum.sales)" accent />
      <StatCard :label="$t('pos.sales.vat')" :value="money(sum.vat_cents, cur)" :hint="$t('pos.sales.invoicesCount', sum.invoices)" />
      <StatCard :label="$t('pos.sales.cashInDrawer')" :value="money(sum.by_method.cash ?? 0, cur)" :hint="$t('pos.sales.netOfChange')" />
      <StatCard :label="$t('pos.sales.nonCash')" :value="money((sum.by_method.card ?? 0) + (sum.by_method.qr ?? 0) + (sum.by_method.transfer ?? 0) + (sum.by_method.other ?? 0), cur)" />
      <StatCard :label="$t('pos.sales.voided')" :value="sum.voids" :hint="money(sum.void_total_cents, cur)" />
    </div>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>

    <div v-if="rows.length" class="card mt-5 overflow-x-auto">
      <table class="table">
        <thead><tr><th>{{ $t('pos.sales.receipt') }}</th><th>{{ $t('pos.sales.time') }}</th><th>{{ $t('pos.sales.items') }}</th><th>{{ $t('pos.sales.payment') }}</th><th>{{ $t('common.total') }}</th><th>{{ $t('pos.sales.taxInvoice') }}</th><th /></tr></thead>
        <tbody>
          <tr v-for="r in rows" :key="r.id" :class="r.status === 'voided' && 'opacity-60'">
            <td>
              <div class="font-bold" :class="r.status === 'voided' && 'line-through'">{{ r.number }}</div>
              <div class="text-xs text-muted">{{ r.cashier }}</div>
            </td>
            <td class="whitespace-nowrap text-muted">{{ qd ? fmtDateTime(r.created_at, 'short', 'short') : fmtTime(r.created_at) }}</td>
            <td>{{ r.units }}</td>
            <td class="text-xs">{{ r.payments.map((p) => methodLabel(p.method)).join(' + ') }}</td>
            <td class="font-bold tabular-nums">{{ money(r.total_cents, cur) }}</td>
            <td>
              <template v-if="r.invoice_number"><span class="font-semibold">{{ r.invoice_number }}</span><div class="max-w-40 truncate text-xs text-muted">{{ r.customer_name }}</div></template>
              <StatusBadge v-else-if="r.status === 'voided'" status="voided" />
              <span v-else class="text-muted">—</span>
            </td>
            <td class="whitespace-nowrap text-right">
              <UButton color="neutral" variant="soft" size="sm" type="submit" @click="print(`/print/receipt/${r.id}?w=${paperWidth}`)">{{ $t('pos.sales.receipt') }}</UButton>
              <UButton color="neutral" variant="soft" size="sm" type="submit" v-if="r.invoice_number" class="ml-1" @click="open(`/print/invoice/${r.id}`)">{{ $t('pos.sales.invoice') }}</UButton>
              <UButton color="neutral" variant="soft" size="sm" type="submit" v-else-if="r.status === 'completed'" class="ml-1" @click="invoicing = r; inv.name = r.customer_name">{{ $t('pos.sales.issueInvoice') }}</UButton>
              <UButton color="neutral" variant="soft" size="sm" type="submit" v-if="r.status === 'completed' && can('pos_void')" class="ml-1 text-brand-700" @click="voiding = r">{{ $t('pos.sales.void') }}</UButton>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <EmptyState v-else class="mt-5" :title="$t('pos.sales.emptyTitle')" :text="$t('pos.sales.emptyText')">
      <UButton to="/pos">{{ $t('pos.sales.openPos') }}</UButton>
    </EmptyState>

    <Teleport to="body">
      <div v-if="voiding" class="fixed inset-0 z-50 grid place-items-center bg-night/40 p-4" @click.self="voiding = null">
        <form class="card w-full max-w-md space-y-3 p-6 shadow-2xl" @submit.prevent="doVoid">
          <div class="text-lg font-bold">{{ $t('pos.sales.voidTitle', { number: voiding.number }) }}</div>
          <p class="text-sm text-muted">{{ $t('pos.sales.voidText') }}</p>
          <UInput v-model="voidReason" required :placeholder="$t('pos.sales.voidReason')" autofocus />
          <div class="flex justify-end gap-2">
            <UButton color="neutral" variant="soft" type="button" @click="voiding = null">{{ $t('common.cancel') }}</UButton>
            <UButton color="neutral" variant="ghost" type="submit" class="bg-brand-700 text-white hover:bg-brand-600">{{ $t('pos.sales.voidSale') }}</UButton>
          </div>
        </form>
      </div>
      <div v-if="invoicing" class="fixed inset-0 z-50 grid place-items-center bg-night/40 p-4" @click.self="invoicing = null">
        <form class="card w-full max-w-md space-y-3 p-6 shadow-2xl" @submit.prevent="doInvoice">
          <div class="text-lg font-bold">{{ $t('pos.sales.invoiceTitle', { number: invoicing.number }) }}</div>
          <UInput v-model="inv.name" required :placeholder="$t('pos.customerNameReq')" autofocus />
          <div class="flex gap-2"><UInput v-model="inv.tax_id" :placeholder="$t('pos.taxId')" /><UInput v-model="inv.branch" :placeholder="$t('pos.branch')" /></div>
          <UTextarea v-model="inv.address" required :rows="3" :placeholder="$t('pos.addressReq')" />
          <UInput v-model="inv.phone" :placeholder="$t('common.phone')" />
          <div class="flex justify-end gap-2">
            <UButton color="neutral" variant="soft" type="button" @click="invoicing = null">{{ $t('common.cancel') }}</UButton>
            <UButton type="submit">{{ $t('pos.issuePrint') }}</UButton>
          </div>
        </form>
      </div>
    </Teleport>
  </div>
</template>
