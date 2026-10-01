<script setup lang="ts">
import type { CodRiskAnchor, CodRiskCustomer, CodRiskReport } from '../../../utils/codRisk'

definePageMeta({ layout: 'admin', middleware: 'admin' })
const api = useApi()
const { t } = useI18n()
const route = useRoute()

type Tab = 'reports' | 'customers' | 'ledger'
const tabs: Tab[] = ['reports', 'customers', 'ledger']
const tab = ref<Tab>(tabs.includes(route.query.tab as Tab) ? (route.query.tab as Tab) : 'reports')
watch(tab, (v) => navigateTo({ query: { tab: v } }, { replace: true }))

const q = ref('')
const search = ref('')

// ---- reports
const statuses = ['pending', 'confirmed', 'dismissed', 'withdrawn', 'all'] as const
const status = ref<(typeof statuses)[number]>('pending')
const { data: reports } = await useAsyncData(
  'codrisk-admin-reports',
  () => api<{ items: CodRiskReport[]; counts: Record<string, number> }>('/admin/cod-risk/reports', { query: { status: status.value, q: search.value || undefined } }),
  { watch: [status, search], default: () => ({ items: [] as CodRiskReport[], counts: {} as Record<string, number> }) },
)
const total = computed(() => Object.values(reports.value.counts).reduce((a, n) => a + n, 0))

// ---- customers
const level = ref('flagged')
const { data: customers } = await useLazyAsyncData(
  'codrisk-admin-customers',
  () => api<CodRiskCustomer[]>('/admin/cod-risk/customers', { query: { level: level.value, q: search.value || undefined } }),
  { watch: [level, search], default: () => [] as CodRiskCustomer[] },
)

// ---- ledger
interface ChainStatus {
  height: number
  tip_hash: string
  valid: boolean
  broken: { height: number; reason: string } | null
  unanchored: number
  anchors: CodRiskAnchor[]
  recent: { height: number; hash: string; event: string; at: string; anchor_id: number | null }[]
  driver: string
  anchoring: boolean
  ledger: string
}
const { data: chain, refresh: refreshChain, status: chainLoad } = await useLazyAsyncData('codrisk-admin-chain', () => api<ChainStatus>('/admin/cod-risk/chain'), {
  immediate: tab.value === 'ledger',
})
watch(tab, (v) => v === 'ledger' && !chain.value && refreshChain())
const anchorMsg = ref('')
const anchorErr = ref('')
const anchoring = ref(false)
async function anchorNow() {
  anchorMsg.value = anchorErr.value = ''
  anchoring.value = true
  try {
    const { anchor: a } = await api<{ anchor: CodRiskAnchor | null }>('/admin/cod-risk/chain/anchor', { method: 'POST' })
    if (!a) anchorMsg.value = t('codrisk.admin.chain.nothing')
    else if (a.status === 'anchored') anchorMsg.value = t('codrisk.admin.chain.anchored', { from: a.from_height, to: a.to_height, tx: shortHash(a.tx_ref, 14) })
    else anchorErr.value = t('codrisk.admin.chain.failed', { error: a.error })
    await refreshChain()
  } catch (e) {
    anchorErr.value = apiError(e)
  } finally {
    anchoring.value = false
  }
}
const proofOf = ref<number | null>(null)
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('codrisk.admin.title') }}</h1>
    <p class="text-sm text-muted">{{ $t('codrisk.admin.subtitle') }}</p>

    <div class="mt-5 flex flex-wrap items-center gap-3">
      <div class="flex flex-wrap rounded-2xl bg-surface p-1 text-sm font-semibold shadow-card">
        <button v-for="s in tabs" :key="s" class="rounded-xl px-3.5 py-1.5" :class="tab === s && 'bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]'" @click="tab = s">
          {{ $t(`codrisk.admin.tabs.${s}`) }}
          <span v-if="s === 'reports' && reports.counts.pending" class="ml-1 rounded-md bg-white/25 px-1.5 text-[11px]">{{ reports.counts.pending }}</span>
        </button>
      </div>
      <form v-if="tab !== 'ledger'" class="ml-auto" @submit.prevent="search = q.trim()">
        <UInput v-model="q" size="md" class="w-72" icon="i-lucide-search" :placeholder="$t('codrisk.admin.search')" :aria-label="$t('codrisk.admin.search')" />
      </form>
    </div>

    <!-- Reports -->
    <section v-if="tab === 'reports'" class="mt-4">
      <div class="flex flex-wrap gap-2">
        <button v-for="s in statuses" :key="s" class="chip border px-3 py-1.5" :class="status === s ? 'border-brand-500 bg-brand-500 text-white' : 'border-transparent bg-surface shadow-card hover:text-brand-500'" @click="status = s">
          {{ s === 'all' ? $t('common.all') : $t(`codrisk.status.${s}`) }} <span class="opacity-60">{{ s === 'all' ? total : reports.counts[s] ?? 0 }}</span>
        </button>
      </div>
      <div v-if="reports.items.length" class="card mt-3 overflow-x-auto">
        <table class="table">
          <thead>
            <tr>
              <th>{{ $t('codrisk.admin.cols.customer') }}</th>
              <th>{{ $t('codrisk.admin.cols.shop') }}</th>
              <th>{{ $t('codrisk.admin.cols.reason') }}</th>
              <th class="text-right">{{ $t('codrisk.admin.cols.amount') }}</th>
              <th>{{ $t('codrisk.admin.cols.history') }}</th>
              <th>{{ $t('codrisk.admin.cols.status') }}</th>
              <th>{{ $t('codrisk.admin.cols.filed') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in reports.items" :key="r.id">
              <td>
                <NuxtLink :to="`/admin/cod-risk/${r.customer_key}`" class="font-semibold hover:underline">{{ r.customer_name || '—' }}</NuxtLink>
                <div class="text-xs text-muted">{{ r.phone }}</div>
              </td>
              <td>{{ r.shop_name }}<div class="font-mono text-xs text-muted">{{ r.order_number }}</div></td>
              <td>{{ $t(`codrisk.reason.${r.reason}`) }}<div v-if="r.note" class="max-w-xs truncate text-xs text-muted" :title="r.note">{{ r.note }}</div></td>
              <td class="text-right tabular-nums">{{ money(r.amount_cents, r.currency) }}</td>
              <td>
                <CodRiskLevelBadge :level="r.level" />
                <div class="text-[11px] text-muted">{{ $t('codrisk.admin.history', { confirmed: r.customer_confirmed ?? 0, total: r.customer_reports ?? 0 }) }}</div>
              </td>
              <td><UBadge :color="reportColor(r.status)" variant="soft" size="sm">{{ $t(`codrisk.status.${r.status}`) }}</UBadge></td>
              <td class="text-xs text-muted">{{ ago(r.created_at) }}<div><NuxtLink :to="`/admin/cod-risk/${r.customer_key}`" class="font-semibold text-brand-700 hover:underline">{{ $t('codrisk.admin.open') }} →</NuxtLink></div></td>
            </tr>
          </tbody>
        </table>
      </div>
      <EmptyState v-else class="mt-3" icon="i-lucide-shield-check" :title="$t('codrisk.admin.reportsEmpty')" />
    </section>

    <!-- Customers -->
    <section v-if="tab === 'customers'" class="mt-4">
      <div class="flex flex-wrap gap-2">
        <button v-for="l in ['flagged', 'all', ...RISK_LEVELS]" :key="l" class="chip border px-3 py-1.5" :class="level === l ? 'border-brand-500 bg-brand-500 text-white' : 'border-transparent bg-surface shadow-card hover:text-brand-500'" @click="level = l">
          {{ l === 'all' ? $t('common.all') : l === 'flagged' ? $t('codrisk.admin.flagged') : $t(`codrisk.level.${l}`) }}
        </button>
      </div>
      <div v-if="customers.length" class="card mt-3 overflow-x-auto">
        <table class="table">
          <thead>
            <tr>
              <th>{{ $t('codrisk.admin.cols.customer') }}</th>
              <th>{{ $t('codrisk.admin.cols.level') }}</th>
              <th>{{ $t('codrisk.admin.cols.reports') }}</th>
              <th>{{ $t('codrisk.admin.cols.updated') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="c in customers" :key="c.customer_key">
              <td>
                <NuxtLink :to="`/admin/cod-risk/${c.customer_key}`" class="font-semibold hover:underline">{{ c.name || c.phone_hint }}</NuxtLink>
                <div class="text-xs text-muted">{{ c.phone || c.phone_hint }}</div>
              </td>
              <td>
                <CodRiskLevelBadge :level="c.effective_level" />
                <div v-if="c.level_expires_at" class="text-[11px] text-muted">{{ $t(c.effective_level === c.level ? 'codrisk.admin.customer.expires' : 'codrisk.admin.customer.expired', { date: fmtDate(c.level_expires_at) }) }}</div>
              </td>
              <td class="text-sm">
                {{ $t('codrisk.counts', { n: c.confirmed, shops: c.shops }, c.confirmed) }}
                <UBadge v-if="c.pending" color="warning" variant="soft" size="sm" class="ml-1">{{ c.pending }} {{ $t('codrisk.status.pending') }}</UBadge>
              </td>
              <td class="text-xs text-muted">{{ ago(c.updated_at) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
      <EmptyState v-else class="mt-3" icon="i-lucide-users" :title="$t('codrisk.admin.customersEmpty')" />
    </section>

    <!-- Ledger -->
    <section v-if="tab === 'ledger'" class="mt-4 space-y-4">
      <div v-if="chainLoad === 'pending' && !chain" class="card p-6 text-sm text-muted">{{ $t('common.loading') }}</div>
      <template v-if="chain">
        <UAlert v-if="chain.valid" color="success" variant="soft" icon="i-lucide-shield-check" :title="$t('codrisk.admin.chain.valid')" />
        <UAlert v-else color="error" variant="solid" icon="i-lucide-shield-x" :title="$t('codrisk.admin.chain.broken', { height: chain.broken?.height ?? 0, reason: chain.broken?.reason ?? '' })" />
        <div class="grid gap-3 sm:grid-cols-3">
          <div class="card p-4"><div class="text-xs font-semibold uppercase tracking-wide text-muted">{{ $t('codrisk.admin.chain.height') }}</div><div class="mt-1 text-2xl font-extrabold tabular-nums">{{ num(chain.height) }}</div></div>
          <div class="card p-4"><div class="text-xs font-semibold uppercase tracking-wide text-muted">{{ $t('codrisk.admin.chain.unanchored') }}</div><div class="mt-1 text-2xl font-extrabold tabular-nums">{{ num(chain.unanchored) }}</div></div>
          <div class="card p-4"><div class="text-xs font-semibold uppercase tracking-wide text-muted">{{ $t('codrisk.admin.chain.tip') }}</div><div class="mt-2 break-all font-mono text-xs">{{ chain.tip_hash }}</div></div>
        </div>
        <div class="card flex flex-wrap items-center gap-3 p-4">
          <div class="min-w-0 flex-1">
            <div class="font-bold">{{ $t('codrisk.admin.chain.driver') }}</div>
            <div class="text-sm text-muted">{{ chain.anchoring ? $t('codrisk.admin.chain.driverOn', { ledger: chain.ledger }) : $t('codrisk.admin.chain.driverOff') }}</div>
            <p v-if="anchorMsg" class="mt-1 text-sm text-mint-500">{{ anchorMsg }}</p>
            <p v-if="anchorErr" class="mt-1 text-sm text-brand-700">{{ anchorErr }}</p>
          </div>
          <UButton v-if="chain.anchoring" icon="i-lucide-anchor" :loading="anchoring" :disabled="!chain.unanchored" @click="anchorNow">{{ $t('codrisk.admin.chain.anchorNow') }}</UButton>
        </div>
        <div v-if="chain.anchors.length" class="card overflow-x-auto">
          <div class="border-b border-line px-4 py-3 font-bold">{{ $t('codrisk.admin.chain.anchors') }}</div>
          <table class="table">
            <thead><tr><th>{{ $t('codrisk.admin.chain.cols.blocks') }}</th><th>{{ $t('codrisk.admin.chain.cols.root') }}</th><th>{{ $t('codrisk.admin.chain.cols.tx') }}</th><th>{{ $t('codrisk.admin.chain.cols.status') }}</th><th>{{ $t('codrisk.admin.chain.cols.at') }}</th></tr></thead>
            <tbody>
              <tr v-for="a in chain.anchors" :key="a.id">
                <td class="font-mono text-xs">#{{ a.from_height }}–#{{ a.to_height }}</td>
                <td class="font-mono text-xs" :title="a.merkle_root">{{ shortHash(a.merkle_root, 14) }}</td>
                <td class="font-mono text-xs" :title="a.tx_ref || a.error">{{ a.tx_ref ? shortHash(a.tx_ref, 14) : a.error.slice(0, 60) }}</td>
                <td><UBadge :color="a.status === 'anchored' ? 'success' : 'error'" variant="soft" size="sm">{{ $t(`codrisk.admin.chain.anchorStatus.${a.status}`) }}</UBadge></td>
                <td class="text-xs text-muted">{{ fmtDateTime(a.created_at) }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <div v-if="chain.recent.length" class="card overflow-x-auto">
          <div class="border-b border-line px-4 py-3 font-bold">{{ $t('codrisk.admin.chain.recent') }}</div>
          <table class="table">
            <thead><tr><th>{{ $t('codrisk.admin.chain.cols.height') }}</th><th>{{ $t('codrisk.admin.chain.cols.event') }}</th><th>{{ $t('codrisk.admin.chain.cols.hash') }}</th><th>{{ $t('codrisk.admin.chain.cols.at') }}</th><th /></tr></thead>
            <tbody>
              <tr v-for="b in chain.recent" :key="b.height">
                <td class="font-mono text-xs">#{{ b.height }}</td>
                <td>{{ $t(eventKey(b.event)) }}</td>
                <td class="font-mono text-xs" :title="b.hash">{{ shortHash(b.hash, 16) }}</td>
                <td class="text-xs text-muted">{{ fmtDateTime(b.at) }}<UIcon v-if="b.anchor_id" name="i-lucide-anchor" class="ml-1 size-3 text-mint-500" /></td>
                <td class="text-right"><UButton size="xs" color="neutral" variant="ghost" icon="i-lucide-file-check" @click="proofOf = b.height">{{ $t('codrisk.admin.customer.proof') }}</UButton></td>
              </tr>
            </tbody>
          </table>
        </div>
      </template>
    </section>

    <CodRiskProofModal v-if="proofOf" :height="proofOf" @close="proofOf = null" />
  </div>
</template>
