<script setup lang="ts">
/** Admin (tax agent): generate monthly filings from every core's sales, submit, mark paid, export; edit rates + policy. */
import { FILING_COLOR, monthLabel, sourceLabel, type Filing, type TaxSettings } from '#finance/utils/finance'

definePageMeta({ layout: 'admin', middleware: 'admin' })
const api = useApi()
const now = new Date(Date.now() + 7 * 3600_000)
const lastMonth = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth() - 1, 1)).toISOString().slice(0, 7)
const period = ref(lastMonth)
interface List {
  period: string
  can_generate: boolean
  mandated_shops: number
  totals: { currency: string; gross_cents: number; vat_cents: number; ecommerce_tax_cents: number; total_due_cents: number }[]
  filings: Filing[]
  generated?: { drafts: number; locked: number }
}
const { data, refresh } = await useAsyncData('admin-filings', () => api<List>('/admin/finance/filings', { query: { period: period.value } }), { watch: [period] })
const msg = ref('')
const error = ref('')
async function generate() {
  msg.value = error.value = ''
  try {
    const r = await api<List>('/admin/finance/filings/generate', { method: 'POST', body: { period: period.value } })
    data.value = r
    msg.value = tr('finance.admin.generated', { n: r.generated?.drafts ?? 0, locked: r.generated?.locked ?? 0 })
  } catch (e) {
    error.value = apiError(e)
  }
}
const refs = reactive<Record<string, string>>({})
const notes = reactive<Record<string, string>>({})
const rowError = reactive<Record<string, string>>({})
async function act(f: Filing, action: string) {
  rowError[f.id] = ''
  try {
    await api(`/admin/finance/filings/${f.id}`, { method: 'PUT', body: { action, reference_no: refs[f.id] ?? '', note: notes[f.id] ?? '' } })
    await refresh()
  } catch (e) {
    rowError[f.id] = apiError(e)
  }
}
const open = ref<string | null>(null)
async function exportCsv() {
  const csv = await api<string>('/admin/finance/filings/export', { query: { period: period.value }, responseType: 'text' as never })
  const url = URL.createObjectURL(new Blob([csv], { type: 'text/csv;charset=utf-8' }))
  const a = Object.assign(document.createElement('a'), { href: url, download: `zaokaiy-tax-${period.value}.csv` })
  a.click()
  URL.revokeObjectURL(url)
}

// settings
const settingsOpen = ref(false)
const s = reactive({ vat: 10 as number | string, ecommerce: 0 as number | string, due_day: 15 as number | string, tax_office: '', agent_name: '', agent_tax_id: '', policy_en: '', policy_lo: '' })
const version = ref('')
async function loadSettings() {
  const r = await api<TaxSettings>('/admin/finance/settings')
  Object.assign(s, { vat: r.vat_bps / 100, ecommerce: r.ecommerce_tax_bps / 100, due_day: r.due_day, tax_office: r.tax_office, agent_name: r.agent_name, agent_tax_id: r.agent_tax_id, policy_en: r.policy_en, policy_lo: r.policy_lo })
  version.value = r.policy_version
  settingsOpen.value = true
}
const settingsError = ref('')
async function saveSettings() {
  settingsError.value = ''
  try {
    const r = await api<TaxSettings>('/admin/finance/settings', {
      method: 'PUT',
      body: { vat_bps: Math.round(Number(s.vat) * 100), ecommerce_tax_bps: Math.round(Number(s.ecommerce) * 100), due_day: Number(s.due_day), tax_office: s.tax_office,
              agent_name: s.agent_name, agent_tax_id: s.agent_tax_id, policy_en: s.policy_en, policy_lo: s.policy_lo },
    })
    version.value = r.policy_version
    settingsOpen.value = false
  } catch (e) {
    settingsError.value = apiError(e)
  }
}
</script>

<template>
  <div v-if="data">
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 class="page-title">{{ $t('finance.admin.title') }}</h1>
        <p class="text-sm text-muted">{{ $t('finance.admin.subtitle', { n: data.mandated_shops }) }}</p>
      </div>
      <div class="flex flex-wrap items-end gap-2">
        <div><label class="label" for="tax-period">{{ $t('finance.col.period') }}</label><UInput id="tax-period" v-model="period" type="month" size="md" /></div>
        <UButton :disabled="!data.can_generate" icon="i-lucide-calculator" @click="generate">{{ $t('finance.admin.generate') }}</UButton>
        <UButton color="neutral" variant="soft" icon="i-lucide-download" :disabled="!data.filings.length" @click="exportCsv">CSV</UButton>
        <UButton color="neutral" variant="soft" icon="i-lucide-settings-2" @click="loadSettings">{{ $t('finance.admin.settings') }}</UButton>
      </div>
    </div>
    <p v-if="!data.can_generate" class="mt-3 text-sm text-muted">{{ $t('finance.admin.notEnded') }}</p>
    <p v-if="msg" class="mt-3 text-sm text-mint-500" role="status">{{ msg }}</p>
    <p v-if="error" class="mt-3 text-sm text-brand-700" role="alert">{{ error }}</p>

    <div v-if="data.totals.length" class="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
      <StatCard v-for="t in data.totals" :key="t.currency" :label="$t('finance.admin.totalDue', { cur: t.currency })" :value="money(t.total_due_cents, t.currency)"
        :hint="$t('finance.admin.totalHint', { gross: money(t.gross_cents, t.currency), vat: money(t.vat_cents, t.currency) })" accent />
    </div>

    <EmptyState v-if="!data.filings.length" class="mt-6" :title="$t('finance.admin.emptyTitle', { month: monthLabel(data.period) })" :text="$t('finance.admin.emptyText')" />
    <div class="mt-4 space-y-2">
      <article v-for="f in data.filings" :key="f.id" class="card p-4" :data-testid="`filing-${f.shop_slug}`">
        <div class="flex flex-wrap items-center gap-3">
          <button type="button" class="min-w-0 flex-1 text-left" @click="open = open === f.id ? null : f.id">
            <div class="font-bold">{{ f.shop_name }} <span class="text-xs font-normal text-muted">{{ f.legal_name }} · {{ $t('finance.mandate.taxId', { id: f.tax_id }) }}</span></div>
            <div class="text-xs text-muted">{{ $t('finance.col.gross') }} {{ money(f.gross_cents, f.currency) }} · VAT {{ money(f.vat_cents, f.currency) }} · {{ $t('finance.col.ecommerce') }} {{ money(f.ecommerce_tax_cents, f.currency) }} · {{ $t('finance.admin.dueOn', { date: fmtDate(f.due_on) }) }}</div>
          </button>
          <b>{{ money(f.total_due_cents, f.currency) }}</b>
          <UBadge :color="FILING_COLOR[f.status]" :label="$t(`finance.status.${f.status}`)" />
        </div>
        <div v-if="open === f.id" class="mt-3 border-t border-line pt-3">
          <table class="w-full text-sm">
            <tr v-for="src in f.sources" :key="src.key"><td class="py-1">{{ sourceLabel(src) }} <span class="text-xs text-muted">· {{ $t('finance.preview.count', { n: src.count }) }}</span></td><td class="text-right">{{ money(src.gross_cents, f.currency) }}</td></tr>
            <tr v-if="!f.sources.length"><td class="py-1 text-muted">{{ $t('finance.admin.nilReturn') }}</td></tr>
          </table>
          <p v-if="f.reference_no" class="mt-2 text-sm">{{ $t('finance.col.reference') }}: <b>{{ f.reference_no }}</b></p>
          <p v-if="f.note" class="mt-1 text-sm text-muted">{{ f.note }}</p>
          <div class="mt-3 flex flex-wrap items-center gap-2">
            <UInput v-if="f.status === 'draft'" v-model="refs[f.id]" size="md" class="w-56" :placeholder="$t('finance.admin.reference')" :aria-label="$t('finance.admin.reference')" />
            <UInput v-if="['draft', 'submitted'].includes(f.status)" v-model="notes[f.id]" size="md" class="w-56" :placeholder="$t('finance.admin.note')" :aria-label="$t('finance.admin.note')" />
            <UButton v-if="f.status === 'draft'" size="sm" @click="act(f, 'submit')">{{ $t('finance.admin.submit') }}</UButton>
            <UButton v-if="f.status === 'submitted'" size="sm" color="success" @click="act(f, 'paid')">{{ $t('finance.admin.paid') }}</UButton>
            <UButton v-if="['draft', 'submitted'].includes(f.status)" size="sm" color="error" variant="soft" @click="act(f, 'void')">{{ $t('finance.admin.void') }}</UButton>
            <UButton v-if="f.status === 'void'" size="sm" color="neutral" variant="soft" @click="act(f, 'reopen')">{{ $t('finance.admin.reopen') }}</UButton>
          </div>
          <p v-if="rowError[f.id]" class="mt-2 text-sm text-brand-700" role="alert">{{ rowError[f.id] }}</p>
        </div>
      </article>
    </div>

    <UModal v-model:open="settingsOpen" :title="$t('finance.admin.settings')" :ui="{ content: 'max-w-2xl' }">
      <template #body>
        <form class="space-y-3" @submit.prevent="saveSettings">
          <div class="grid gap-3 sm:grid-cols-3">
            <div><label class="label" for="st-vat">{{ $t('finance.rates.vat') }} (%)</label><UInput id="st-vat" v-model.number="s.vat" type="number" min="0" max="50" step="0.01" /></div>
            <div><label class="label" for="st-etax">{{ $t('finance.rates.ecommerce') }} (%)</label><UInput id="st-etax" v-model.number="s.ecommerce" type="number" min="0" max="50" step="0.01" /></div>
            <div><label class="label" for="st-due">{{ $t('finance.admin.dueDayLabel') }}</label><UInput id="st-due" v-model.number="s.due_day" type="number" min="1" max="28" /></div>
          </div>
          <div class="grid gap-3 sm:grid-cols-3">
            <div><label class="label" for="st-agent">{{ $t('finance.admin.agentName') }}</label><UInput id="st-agent" v-model="s.agent_name" /></div>
            <div><label class="label" for="st-tin">{{ $t('finance.admin.agentTin') }}</label><UInput id="st-tin" v-model="s.agent_tax_id" /></div>
            <div><label class="label" for="st-office">{{ $t('finance.admin.taxOffice') }}</label><UInput id="st-office" v-model="s.tax_office" /></div>
          </div>
          <div><label class="label" for="st-pen">{{ $t('finance.admin.policyEn') }}</label><UTextarea id="st-pen" v-model="s.policy_en" :rows="6" /></div>
          <div><label class="label" for="st-plo">{{ $t('finance.admin.policyLo') }}</label><UTextarea id="st-plo" v-model="s.policy_lo" :rows="6" /></div>
          <p class="text-xs text-muted">{{ $t('finance.admin.policyHint', { v: version }) }}</p>
          <p v-if="settingsError" class="text-sm text-brand-700" role="alert">{{ settingsError }}</p>
          <UButton type="submit" block>{{ $t('common.save') }}</UButton>
        </form>
      </template>
    </UModal>
  </div>
</template>
