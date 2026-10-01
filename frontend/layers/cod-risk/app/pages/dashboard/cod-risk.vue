<script setup lang="ts">
import type { CodRiskOrder, CodRiskReport, RiskLevel } from '../../utils/codRisk'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { t } = useI18n()
const { shopId, shop } = useShop()
const { carrierName } = useCarriers()
const { enabled, status: modules } = useModules()

type Tab = 'unshipped' | 'refused' | 'reports' | 'check'
const tabs: Tab[] = ['unshipped', 'refused', 'reports', 'check']
const route = useRoute()
const tab = ref<Tab>(tabs.includes(route.query.tab as Tab) ? (route.query.tab as Tab) : 'unshipped')
watch(tab, (v) => navigateTo({ query: { ...route.query, tab: v } }, { replace: true }))

const { data: unshipped } = await useAsyncData(
  'codrisk-unshipped',
  () => api<{ rows: CodRiskOrder[] }>(`/shops/${shopId.value}/cod-risk/unshipped`).catch(() => ({ rows: [] })),
  { watch: [shopId], default: () => ({ rows: [] as CodRiskOrder[] }) },
)
const { data: refused, refresh: refreshRefused } = await useAsyncData(
  'codrisk-refused',
  () => api<{ rows: CodRiskOrder[]; window_days: number }>(`/shops/${shopId.value}/cod-risk/refused`).catch(() => ({ rows: [], window_days: 30 })),
  { watch: [shopId], default: () => ({ rows: [] as CodRiskOrder[], window_days: 30 }) },
)
const { data: reports, refresh: refreshReports } = await useAsyncData(
  'codrisk-reports',
  () => api<CodRiskReport[]>(`/shops/${shopId.value}/cod-risk/reports`).catch(() => []),
  { watch: [shopId], default: () => [] as CodRiskReport[] },
)
const risky = computed(() => unshipped.value.rows.filter((r) => r.risk && r.risk.level !== 'none').length)
const toReport = computed(() => refused.value.rows.filter((r) => r.reportable).length)
const counts = computed<Record<Tab, number>>(() => ({ unshipped: risky.value, refused: toReport.value, reports: 0, check: 0 }))

const msg = ref('')
const error = ref('')
const busy = ref(false)

// ---- report modal
const target = ref<CodRiskOrder | null>(null)
const form = reactive({ reason: 'refused', note: '' })
function openReport(o: CodRiskOrder) {
  target.value = o
  form.reason = 'refused'
  form.note = ''
  error.value = msg.value = ''
}
async function sendReport() {
  if (!target.value) return
  busy.value = true
  error.value = ''
  try {
    await api(`/shops/${shopId.value}/cod-risk/reports`, {
      method: 'POST',
      body: { kind: target.value.kind, order_id: target.value.id, reason: form.reason, note: form.note.trim() },
    })
    target.value = null
    msg.value = t('codrisk.seller.filed')
    await Promise.all([refreshRefused(), refreshReports()])
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
async function withdraw(r: CodRiskReport) {
  if (!confirm(t('codrisk.seller.withdrawConfirm'))) return
  error.value = msg.value = ''
  try {
    await api(`/cod-risk/reports/${r.id}/withdraw`, { method: 'POST' })
    msg.value = t('codrisk.seller.withdrawn')
    await Promise.all([refreshRefused(), refreshReports()])
  } catch (e) {
    error.value = apiError(e)
  }
}

// ---- lookup
interface CheckResult { input: string; valid: boolean; phone?: string; level?: RiskLevel; confirmed_reports?: number; shops?: number }
const phones = ref('')
const results = ref<CheckResult[]>([])
async function check() {
  const list = phones.value.split(/[\n,;]+/).map((p) => p.trim()).filter(Boolean)
  if (!list.length) return
  busy.value = true
  error.value = ''
  try {
    results.value = (await api<{ results: CheckResult[] }>(`/shops/${shopId.value}/cod-risk/check`, { method: 'POST', body: { phones: list.slice(0, 50) } })).results
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
const orderLink = (o: { kind: string; id: string }) => (o.kind === 'social' ? `/dashboard/social/orders/${o.id}` : '/dashboard/orders')
const cur = computed(() => shop.value?.currency ?? 'LAK')
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('codrisk.seller.title') }}</h1>
    <p class="mt-1 max-w-3xl text-sm text-muted">{{ $t('codrisk.seller.intro') }}</p>

    <UAlert v-if="modules && Object.keys(modules).length && !enabled('cod_risk')" class="mt-4" color="warning" variant="soft" icon="i-lucide-power-off" :title="$t('codrisk.disabled')" />

    <div class="mt-5 flex flex-wrap rounded-2xl bg-surface p-1 text-sm font-semibold shadow-card sm:inline-flex">
      <button v-for="s in tabs" :key="s" class="flex items-center gap-1.5 rounded-xl px-3.5 py-1.5" :class="tab === s && 'bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]'" @click="tab = s">
        {{ $t(`codrisk.seller.tabs.${s}`) }}
        <span v-if="counts[s]" class="rounded-md px-1.5 text-[11px]" :class="tab === s ? 'bg-white/25' : 'bg-brand-50 text-brand-700'">{{ counts[s] }}</span>
      </button>
    </div>

    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>
    <p v-if="msg" class="mt-3 text-sm text-mint-500">{{ msg }}</p>

    <!-- Before you ship -->
    <section v-if="tab === 'unshipped'" class="mt-4">
      <p class="text-sm text-muted">{{ $t('codrisk.seller.unshippedIntro') }}</p>
      <div v-if="unshipped.rows.length" class="card mt-3 overflow-x-auto">
        <table class="table">
          <thead>
            <tr>
              <th>{{ $t('codrisk.seller.cols.order') }}</th>
              <th>{{ $t('codrisk.seller.cols.customer') }}</th>
              <th class="text-right">{{ $t('codrisk.seller.cols.amount') }}</th>
              <th>{{ $t('codrisk.seller.cols.risk') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="o in unshipped.rows" :key="`${o.kind}:${o.id}`" :class="o.risk && o.risk.level !== 'none' && 'bg-brand-50/40'">
              <td><NuxtLink :to="orderLink(o)" class="font-mono font-bold hover:underline">{{ o.number }}</NuxtLink><div class="text-xs text-muted">{{ fmtDate(o.created_at) }}</div></td>
              <td>{{ o.customer || '—' }}<div class="text-xs text-muted"><a v-if="o.phone" :href="`tel:${o.phone}`" class="hover:underline">{{ o.phone }}</a></div></td>
              <td class="text-right font-semibold tabular-nums">{{ money(o.cod_amount_cents, o.currency) }}</td>
              <td>
                <CodRiskLevelBadge v-if="o.risk" :level="o.risk.level" :confirmed="o.risk.confirmed_reports" :shops="o.risk.shops" help />
                <span v-else class="text-xs text-muted">{{ $t('codrisk.seller.noPhone') }}</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <EmptyState v-else class="mt-3" icon="i-lucide-package-check" :title="$t('codrisk.seller.tabs.unshipped')" :text="$t('codrisk.seller.unshippedEmpty')" />
    </section>

    <!-- Refused parcels -->
    <section v-if="tab === 'refused'" class="mt-4">
      <p class="text-sm text-muted">{{ $t('codrisk.seller.refusedIntro', { days: refused.window_days }) }}</p>
      <div v-if="refused.rows.length" class="card mt-3 overflow-x-auto">
        <table class="table">
          <thead>
            <tr>
              <th>{{ $t('codrisk.seller.cols.order') }}</th>
              <th>{{ $t('codrisk.seller.cols.customer') }}</th>
              <th>{{ $t('codrisk.seller.cols.courier') }}</th>
              <th class="text-right">{{ $t('codrisk.seller.cols.amount') }}</th>
              <th>{{ $t('codrisk.seller.cols.returned') }}</th>
              <th>{{ $t('codrisk.seller.cols.risk') }}</th>
              <th class="text-right">{{ $t('codrisk.seller.cols.report') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="o in refused.rows" :key="`${o.kind}:${o.id}`">
              <td><NuxtLink :to="orderLink(o)" class="font-mono font-bold hover:underline">{{ o.number }}</NuxtLink></td>
              <td>{{ o.customer || '—' }}<div class="text-xs text-muted">{{ o.phone || '—' }}</div></td>
              <td>{{ o.carrier_code ? carrierName(o.carrier_code) : '—' }}<div class="font-mono text-xs text-muted">{{ o.tracking_no || '—' }}</div></td>
              <td class="text-right font-semibold tabular-nums">{{ money(o.cod_amount_cents, o.currency) }}</td>
              <td class="text-xs text-muted">{{ ago(o.updated_at) }}</td>
              <td><CodRiskLevelBadge v-if="o.risk" :level="o.risk.level" :confirmed="o.risk.confirmed_reports" :shops="o.risk.shops" /></td>
              <td class="text-right">
                <UBadge v-if="o.report" :color="reportColor(o.report.status)" variant="soft" size="sm">{{ $t(`codrisk.status.${o.report.status}`) }}</UBadge>
                <UButton v-else-if="o.reportable" size="sm" color="error" variant="soft" icon="i-lucide-flag" @click="openReport(o)">{{ $t('codrisk.seller.report') }}</UButton>
                <span v-else class="text-xs text-muted">{{ o.phone ? $t('codrisk.seller.tooLate') : $t('codrisk.seller.noPhone') }}</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <EmptyState v-else class="mt-3" icon="i-lucide-party-popper" :title="$t('codrisk.seller.tabs.refused')" :text="$t('codrisk.seller.refusedEmpty')" />
    </section>

    <!-- My reports -->
    <section v-if="tab === 'reports'" class="mt-4">
      <div v-if="reports.length" class="card overflow-x-auto">
        <table class="table">
          <thead>
            <tr>
              <th>{{ $t('codrisk.seller.cols.order') }}</th>
              <th>{{ $t('codrisk.seller.cols.customer') }}</th>
              <th>{{ $t('codrisk.seller.cols.reason') }}</th>
              <th class="text-right">{{ $t('codrisk.seller.cols.amount') }}</th>
              <th>{{ $t('codrisk.seller.cols.status') }}</th>
              <th>{{ $t('codrisk.seller.cols.filed') }}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in reports" :key="r.id">
              <td class="font-mono font-bold">{{ r.order_number }}</td>
              <td>{{ r.customer_name || '—' }}<div class="text-xs text-muted">{{ r.phone }}</div></td>
              <td>{{ $t(`codrisk.reason.${r.reason}`) }}<div v-if="r.note" class="max-w-xs truncate text-xs text-muted" :title="r.note">{{ r.note }}</div></td>
              <td class="text-right tabular-nums">{{ money(r.amount_cents, r.currency || cur) }}</td>
              <td>
                <UBadge :color="reportColor(r.status)" variant="soft" size="sm">{{ $t(`codrisk.status.${r.status}`) }}</UBadge>
                <div v-if="r.decision_note" class="mt-0.5 max-w-xs text-[11px] text-muted">{{ $t('codrisk.seller.adminNote', { note: r.decision_note }) }}</div>
              </td>
              <td class="text-xs text-muted">{{ fmtDate(r.created_at) }}<div v-if="r.block_height" class="font-mono">{{ $t('codrisk.seller.ledger', { height: r.block_height }) }}</div></td>
              <td class="text-right"><UButton v-if="r.status === 'pending'" size="xs" color="neutral" variant="ghost" @click="withdraw(r)">{{ $t('codrisk.seller.withdraw') }}</UButton></td>
            </tr>
          </tbody>
        </table>
      </div>
      <EmptyState v-else icon="i-lucide-flag" :title="$t('codrisk.seller.tabs.reports')" :text="$t('codrisk.seller.reportsEmpty')" />
    </section>

    <!-- Check a number -->
    <section v-if="tab === 'check'" class="mt-4 grid gap-4 lg:grid-cols-[minmax(0,22rem)_1fr]">
      <form class="card space-y-3 p-4" @submit.prevent="check">
        <p class="text-sm text-muted">{{ $t('codrisk.seller.checkIntro') }}</p>
        <UTextarea v-model="phones" :rows="5" class="w-full" :placeholder="$t('codrisk.seller.checkPh')" :aria-label="$t('common.phone')" />
        <UButton type="submit" icon="i-lucide-search" :loading="busy" :disabled="!phones.trim()">{{ $t('codrisk.seller.checkBtn') }}</UButton>
      </form>
      <div v-if="results.length" class="card divide-y divide-line/70">
        <div v-for="(r, i) in results" :key="i" class="flex flex-wrap items-center justify-between gap-3 px-4 py-3">
          <div>
            <div class="font-semibold">{{ r.phone || r.input }}</div>
            <div v-if="!r.valid" class="text-xs text-brand-700">{{ $t('codrisk.seller.invalid') }}</div>
            <div v-else class="text-xs text-muted">{{ $t(`codrisk.levelHelp.${r.level}`) }}</div>
          </div>
          <CodRiskLevelBadge v-if="r.valid" :level="r.level" :confirmed="r.confirmed_reports" :shops="r.shops" />
        </div>
      </div>
    </section>

    <UModal :open="!!target" :title="target ? $t('codrisk.seller.modalTitle', { number: target.number }) : ''" @update:open="(o) => !o && (target = null)">
      <template #body>
        <form id="codrisk-report" class="space-y-4" @submit.prevent="sendReport">
          <p class="text-sm text-muted">{{ $t('codrisk.seller.modalIntro') }}</p>
          <div v-if="target" class="rounded-xl bg-[var(--field)] p-3 text-sm">
            <div class="font-semibold">{{ target.customer }} · {{ target.phone }}</div>
            <div class="text-muted">{{ target.carrier_code ? carrierName(target.carrier_code) : '—' }} {{ target.tracking_no }} · {{ money(target.cod_amount_cents, target.currency) }}</div>
          </div>
          <fieldset>
            <legend class="label">{{ $t('codrisk.seller.reasonLabel') }}</legend>
            <div class="grid gap-1.5 sm:grid-cols-2">
              <label v-for="r in REPORT_REASONS" :key="r" class="flex cursor-pointer items-center gap-2 rounded-xl border px-3 py-2 text-sm" :class="form.reason === r ? 'border-brand-500 bg-brand-50/60' : 'border-line'">
                <input v-model="form.reason" type="radio" name="reason" :value="r" class="accent-brand-500">
                {{ $t(`codrisk.reason.${r}`) }}
              </label>
            </div>
          </fieldset>
          <div>
            <label for="codrisk-note" class="label">{{ $t('codrisk.seller.noteLabel') }} <span v-if="form.reason === 'other'" class="text-brand-700">· {{ $t('codrisk.seller.noteRequired') }}</span></label>
            <UTextarea id="codrisk-note" v-model="form.note" :rows="3" maxlength="1000" class="w-full" :placeholder="$t('codrisk.seller.notePh')" />
          </div>
          <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
        </form>
      </template>
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="soft" @click="target = null">{{ $t('common.cancel') }}</UButton>
          <UButton type="submit" form="codrisk-report" color="error" icon="i-lucide-flag" :loading="busy" :disabled="form.reason === 'other' && form.note.trim().length < 5">{{ $t('codrisk.seller.send') }}</UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
