<script setup lang="ts">
import type { CodLedger, CodRow, CodStatus } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { t } = useI18n()
const { shopId, shop } = useShop()
const { carriers, carrierName } = useCarriers()
const cur = computed(() => shop.value?.currency ?? 'LAK')

type Tab = '' | Exclude<CodStatus, 'none'>
const tabs: Tab[] = ['', 'pending', 'collected', 'remitted', 'returned']
const status = ref<Tab>('pending')
const carrier = ref('')
const { data: ledger, refresh } = await useAsyncData(
  'cod-ledger',
  () => api<CodLedger>(`/shops/${shopId.value}/cod`, { query: { status: status.value || undefined, carrier: carrier.value || undefined } }),
  { watch: [shopId, status, carrier], default: (): CodLedger => ({ rows: [], totals: [] }) },
)
const rows = computed(() => ledger.value.rows)

const cards: Exclude<CodStatus, 'none'>[] = ['pending', 'collected', 'remitted', 'returned']
const summary = computed(() => {
  const out = Object.fromEntries(cards.map((c) => [c, { count: 0, amount: 0 }])) as Record<string, { count: number; amount: number }>
  for (const r of ledger.value.totals) {
    const s = out[r.cod_status]
    if (s) { s.count += r.count; s.amount += r.amount_cents }
  }
  return out
})
/** What each courier still owes: pending (not yet collected) + collected (not yet transferred). */
const byCourier = computed(() => {
  const m = new Map<string, { code: string | null; pending: number; collected: number; count: number }>()
  for (const r of ledger.value.totals) {
    if (r.cod_status !== 'pending' && r.cod_status !== 'collected') continue
    const k = r.carrier_code ?? ''
    const e = m.get(k) ?? { code: r.carrier_code, pending: 0, collected: 0, count: 0 }
    e[r.cod_status] += r.amount_cents
    e.count += r.count
    m.set(k, e)
  }
  return [...m.values()].sort((a, b) => b.pending + b.collected - (a.pending + a.collected))
})

// ---- selection + bulk actions
const key = (r: CodRow) => `${r.kind}:${r.id}`
const selected = ref<Set<string>>(new Set())
watch(rows, () => (selected.value = new Set()))
const selRows = computed(() => rows.value.filter((r) => selected.value.has(key(r))))
const allSelected = computed(() => rows.value.length > 0 && selRows.value.length === rows.value.length)
function toggle(r: CodRow) {
  const s = new Set(selected.value)
  if (s.has(key(r))) s.delete(key(r))
  else s.add(key(r))
  selected.value = s
}
function toggleAll() {
  selected.value = allSelected.value ? new Set() : new Set(rows.value.map(key))
}
// Cash can only be recorded once the parcel is with the courier (shipped); returns only while undelivered.
const shipped = (r: { order_status: string }) => r.order_status === 'shipped' || r.order_status === 'completed'
const canCollect = computed(() => selRows.value.length > 0 && selRows.value.every((r) => r.cod_status === 'pending' && shipped(r)))
const canRemit = computed(() => selRows.value.length > 0 && selRows.value.every((r) => (r.cod_status === 'pending' || r.cod_status === 'collected') && shipped(r)))
const canReturn = computed(() => selRows.value.length > 0 && selRows.value.every((r) => r.cod_status === 'pending' && r.order_status !== 'completed'))
const selTotal = computed(() => selRows.value.reduce((s, r) => s + r.cod_amount_cents, 0))

const error = ref('')
const msg = ref('')
const busy = ref(false)
const remitOpen = ref(false)
const remitRef = ref('')
async function update(next: 'collected' | 'remitted' | 'returned', reference?: string) {
  error.value = msg.value = ''
  if (next === 'returned' && !confirm(t('ship.cod.confirmReturn', { n: selRows.value.length }, selRows.value.length))) return
  busy.value = true
  try {
    const res = await api<{ updated: number }>(`/shops/${shopId.value}/cod/update`, {
      method: 'POST',
      body: { items: selRows.value.map((r) => ({ kind: r.kind, id: r.id })), status: next, reference },
    })
    msg.value = t('ship.cod.updated', { n: res.updated }, res.updated)
    remitOpen.value = false
    remitRef.value = ''
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
const orderLink = (r: CodRow) => (r.kind === 'social' ? `/dashboard/social/orders/${r.id}` : '/dashboard/orders')
const tabLabel = (s: Tab) => (s ? t(`common.status.${s}`) : t('common.all'))
const tone: Record<string, string> = { pending: 'text-amber-700', collected: 'text-blue-600', remitted: 'text-mint-500', returned: 'text-brand-700' }
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('ship.cod.title') }}</h1>
    <p class="mt-1 max-w-3xl text-sm text-muted">{{ $t('ship.cod.intro') }}</p>

    <div class="mt-5 grid grid-cols-2 gap-3 lg:grid-cols-4">
      <button v-for="c in cards" :key="c" class="card p-4 text-left transition hover:border-ink/40" :class="status === c && 'border-ink'" @click="status = c">
        <div class="text-xs font-semibold uppercase tracking-wide text-muted">{{ $t(`ship.cod.cards.${c}`) }}</div>
        <div class="mt-1 text-xl font-extrabold tabular-nums" :class="tone[c]">{{ money(summary[c]?.amount, cur) }}</div>
        <div class="text-xs text-muted">{{ $t('ship.cod.orders', { n: summary[c]?.count ?? 0 }, summary[c]?.count ?? 0) }}</div>
      </button>
    </div>

    <div class="card mt-4 overflow-x-auto">
      <div class="border-b border-line px-4 py-3 font-bold">{{ $t('ship.cod.byCourier') }}</div>
      <table v-if="byCourier.length" class="table">
        <thead>
          <tr>
            <th class="text-left">{{ $t('ship.cod.cols.courier') }}</th>
            <th class="text-right">{{ $t('ship.cod.cards.pending') }}</th>
            <th class="text-right">{{ $t('ship.cod.cards.collected') }}</th>
            <th class="text-right">{{ $t('ship.cod.outstanding') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="c in byCourier" :key="c.code ?? ''">
            <td>
              <button class="font-semibold hover:underline" @click="carrier = c.code ?? ''">{{ c.code ? carrierName(c.code) : $t('ship.noCourier') }}</button>
              <div class="text-xs text-muted">{{ $t('ship.cod.orders', { n: c.count }, c.count) }}</div>
            </td>
            <td class="text-right tabular-nums">{{ money(c.pending, cur) }}</td>
            <td class="text-right tabular-nums">{{ money(c.collected, cur) }}</td>
            <td class="text-right font-bold tabular-nums">{{ money(c.pending + c.collected, cur) }}</td>
          </tr>
        </tbody>
      </table>
      <p v-else class="px-4 py-3 text-sm text-muted">{{ $t('ship.cod.noOutstanding') }}</p>
    </div>

    <div class="mt-6 flex flex-wrap items-center gap-3">
      <div class="flex flex-wrap rounded-2xl bg-surface p-1 shadow-card text-sm font-semibold">
        <button v-for="s in tabs" :key="s" class="rounded-xl px-3.5 py-1.5" :class="status === s && 'bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]'" @click="status = s">{{ tabLabel(s) }}</button>
      </div>
      <select v-model="carrier" class="input w-48">
        <option value="">{{ $t('ship.cod.allCouriers') }}</option>
        <option v-for="c in carriers" :key="c.code" :value="c.code">{{ carrierName(c.code) }}</option>
      </select>
    </div>

    <div v-if="selRows.length" class="card mt-3 space-y-3 border-ink p-3">
      <div class="flex flex-wrap items-center gap-2">
        <span class="text-sm font-bold">{{ $t('ship.cod.selected', { n: selRows.length }) }} · {{ money(selTotal, cur) }}</span>
        <div class="ml-auto flex flex-wrap gap-2">
          <UButton size="sm" type="submit" :disabled="!canCollect || busy" @click="update('collected')">{{ $t('ship.cod.markCollected') }}</UButton>
          <UButton color="neutral" size="sm" type="submit" :disabled="!canRemit || busy" @click="remitOpen = !remitOpen">{{ $t('ship.cod.markRemitted') }}</UButton>
          <UButton color="neutral" variant="ghost" size="sm" type="submit" class="bg-brand-50 text-brand-700 hover:bg-brand-100" :disabled="!canReturn || busy" @click="update('returned')">{{ $t('ship.cod.markReturned') }}</UButton>
        </div>
      </div>
      <form v-if="remitOpen && canRemit" class="flex flex-wrap items-end gap-2 border-t border-line pt-3" @submit.prevent="update('remitted', remitRef.trim())">
        <div class="min-w-56 flex-1">
          <label class="label">{{ $t('ship.cod.remitRef') }}</label>
          <UInput v-model="remitRef" maxlength="120" :placeholder="$t('ship.cod.remitRefPlaceholder')" />
        </div>
        <UButton color="neutral" type="submit" :disabled="busy">{{ $t('ship.cod.confirmRemit') }}</UButton>
        <UButton color="neutral" variant="soft" type="button" @click="remitOpen = false">{{ $t('common.cancel') }}</UButton>
      </form>
    </div>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>
    <p v-if="msg" class="mt-3 text-sm text-mint-500">{{ msg }}</p>

    <div v-if="rows.length" class="card mt-3 overflow-x-auto">
      <table class="table">
        <thead>
          <tr>
            <th class="w-8"><input type="checkbox" class="size-4 accent-brand-500" :checked="allSelected" :aria-label="$t('ship.cod.selectAll')" @change="toggleAll"></th>
            <th class="text-left">{{ $t('ship.cod.cols.order') }}</th>
            <th class="text-left">{{ $t('ship.cod.cols.customer') }}</th>
            <th class="text-left">{{ $t('ship.cod.cols.courier') }}</th>
            <th class="text-left">{{ $t('ship.cod.cols.tracking') }}</th>
            <th class="text-right">{{ $t('ship.cod.cols.amount') }}</th>
            <th class="text-left">{{ $t('ship.cod.cols.status') }}</th>
            <th class="text-left">{{ $t('ship.cod.cols.shipped') }}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in rows" :key="key(r)" :class="selected.has(key(r)) && 'bg-paper'">
            <td><input type="checkbox" class="size-4 accent-brand-500" :checked="selected.has(key(r))" :aria-label="r.number" @change="toggle(r)"></td>
            <td>
              <NuxtLink :to="orderLink(r)" class="font-mono font-bold hover:underline">{{ r.number }}</NuxtLink>
              <div class="text-xs text-muted">{{ fmtDate(r.created_at) }}</div>
            </td>
            <td>{{ r.customer || '—' }}<div v-if="r.phone" class="text-xs text-muted">{{ r.phone }}</div></td>
            <td>{{ r.carrier_code ? carrierName(r.carrier_code) : '—' }}</td>
            <td class="font-mono text-xs">
              <a v-if="r.tracking_link" :href="r.tracking_link" target="_blank" rel="noopener" class="text-brand-700 hover:underline">{{ r.tracking_no }}</a>
              <span v-else>{{ r.tracking_no || '—' }}</span>
            </td>
            <td class="text-right font-semibold tabular-nums">{{ money(r.cod_amount_cents, r.currency) }}</td>
            <td>
              <StatusBadge :status="r.cod_status" />
              <div v-if="r.cod_remit_ref" class="mt-0.5 text-[11px] text-muted">{{ $t('ship.cod.remitRefShown', { ref: r.cod_remit_ref }) }}</div>
            </td>
            <td class="text-xs text-muted">{{ r.shipped_at ? fmtDate(r.shipped_at) : '—' }}</td>
            <td class="whitespace-nowrap text-right text-xs">
              <NuxtLink :to="orderLink(r)" class="font-semibold hover:underline">{{ $t('ship.cod.openOrder') }}</NuxtLink>
              <span class="mx-1 text-line">|</span>
              <a :href="`/print/label/${r.kind}/${r.id}`" target="_blank" class="font-semibold hover:underline">{{ $t('ship.cod.label') }}</a>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <EmptyState v-else class="mt-3" :title="$t('ship.cod.title')" :text="$t('ship.cod.empty')" />
  </div>
</template>
