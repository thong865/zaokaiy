<script setup lang="ts">
import type { CodRiskCustomerDoc, CodRiskReport, RiskLevel } from '../../../utils/codRisk'

definePageMeta({ layout: 'admin', middleware: 'admin' })
const api = useApi()
const { t } = useI18n()
const { carrierName } = useCarriers()
const route = useRoute()
const key = computed(() => String(route.params.key))

const { data: doc, error: loadError } = await useAsyncData(`codrisk-customer-${key.value}`, () => api<CodRiskCustomerDoc>(`/admin/cod-risk/customers/${key.value}`))
const c = computed(() => doc.value?.customer)
const currency = computed(() => doc.value?.reports[0]?.currency ?? 'LAK')

const msg = ref('')
const error = ref('')
const busy = ref(false)
async function run(fn: () => Promise<CodRiskCustomerDoc>, ok: string) {
  error.value = msg.value = ''
  busy.value = true
  try {
    doc.value = await fn()
    msg.value = ok
    return true
  } catch (e) {
    error.value = apiError(e)
    return false
  } finally {
    busy.value = false
  }
}

// ---- level
const lv = reactive<{ level: RiskLevel; note: string; days: number | null }>({ level: 'none', note: '', days: null })
watch(doc, (d) => {
  if (!d) return
  lv.level = d.customer.effective_level === 'none' ? d.suggested_level : d.customer.effective_level
}, { immediate: true })
async function saveLevel() {
  const ok = await run(
    () => api<CodRiskCustomerDoc>(`/admin/cod-risk/customers/${key.value}/level`, { method: 'POST', body: { level: lv.level, note: lv.note.trim(), expires_days: lv.days || undefined } }),
    t('codrisk.admin.customer.saved'),
  )
  if (ok) lv.note = ''
}

// ---- report decisions (a draft note/level per report)
const reopened = ref<Set<string>>(new Set())
const drafts = reactive<Record<string, { note: string; level: '' | RiskLevel }>>({})
const d = (r: CodRiskReport) => (drafts[r.id] ??= { note: '', level: '' })
watch(doc, (v) => v?.reports.forEach((r) => d(r)), { immediate: true })
const open = (r: CodRiskReport) => r.status === 'pending' || reopened.value.has(r.id)
async function decide(r: CodRiskReport, action: 'confirm' | 'dismiss') {
  const draft = d(r)
  const ok = await run(
    () => api<CodRiskCustomerDoc>(`/admin/cod-risk/reports/${r.id}/decision`, { method: 'POST', body: { action, note: draft.note.trim(), level: draft.level || undefined } }),
    t('codrisk.admin.customer.decided'),
  )
  if (ok) {
    delete drafts[r.id]
    reopened.value.delete(r.id)
  }
}
/** Dismissing, changing a decision, or setting a level needs a note (the API enforces it too). */
const needsNote = (r: CodRiskReport, action: 'confirm' | 'dismiss') => (action === 'dismiss' || r.status !== 'pending' || !!d(r).level) && !d(r).note.trim()
const proofOf = ref<number | null>(null)
</script>

<template>
  <div>
    <NuxtLink to="/admin/cod-risk" class="text-sm font-semibold text-muted hover:text-brand-500">{{ $t('codrisk.admin.customer.back') }}</NuxtLink>
    <p v-if="loadError" class="mt-4 text-sm text-brand-700">{{ apiError(loadError) }}</p>
    <template v-if="doc && c">
      <div class="mt-2 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 class="page-title">{{ c.name || c.phone_hint }}</h1>
          <p class="text-sm text-muted">
            <a v-if="c.phone" :href="`tel:${c.phone}`" class="hover:underline">{{ c.phone }}</a>
            <span v-else>{{ $t('codrisk.admin.customer.phoneHint', { hint: c.phone_hint }) }}</span>
          </p>
          <p class="mt-1 font-mono text-[11px] text-muted" :title="c.customer_key">key {{ shortHash(c.customer_key, 16) }}</p>
        </div>
        <div class="text-right">
          <div class="text-xs font-semibold uppercase tracking-wide text-muted">{{ $t('codrisk.admin.customer.effective') }}</div>
          <CodRiskLevelBadge :level="c.effective_level" help class="mt-1" />
          <div v-if="c.level_expires_at" class="text-[11px] text-muted">{{ $t(c.effective_level === c.level ? 'codrisk.admin.customer.expires' : 'codrisk.admin.customer.expired', { date: fmtDate(c.level_expires_at) }) }}</div>
          <div v-if="c.level_note" class="mt-1 max-w-xs text-xs text-muted">“{{ c.level_note }}”</div>
        </div>
      </div>

      <div class="mt-5 grid grid-cols-2 gap-3 lg:grid-cols-5">
        <div class="card p-4"><div class="text-xs font-semibold uppercase tracking-wide text-muted">{{ $t('codrisk.admin.customer.stats') }}</div><div class="mt-1 text-2xl font-extrabold tabular-nums">{{ doc.stats.confirmed }}</div></div>
        <div class="card p-4"><div class="text-xs font-semibold uppercase tracking-wide text-muted">{{ $t('codrisk.admin.customer.shops') }}</div><div class="mt-1 text-2xl font-extrabold tabular-nums">{{ doc.stats.shops }}</div></div>
        <div class="card p-4"><div class="text-xs font-semibold uppercase tracking-wide text-muted">{{ $t('codrisk.admin.customer.last90') }}</div><div class="mt-1 text-2xl font-extrabold tabular-nums">{{ doc.stats.confirmed_90d }}</div></div>
        <div class="card p-4"><div class="text-xs font-semibold uppercase tracking-wide text-muted">{{ $t('codrisk.admin.customer.pending') }}</div><div class="mt-1 text-2xl font-extrabold tabular-nums text-amber-700">{{ doc.stats.pending }}</div></div>
        <div class="card col-span-2 p-4 lg:col-span-1"><div class="text-xs font-semibold uppercase tracking-wide text-muted">{{ $t('codrisk.admin.customer.refusedValue') }}</div><div class="mt-1 text-xl font-extrabold tabular-nums">{{ money(doc.stats.amount_cents, currency) }}</div></div>
      </div>

      <p v-if="error" class="mt-4 text-sm text-brand-700">{{ error }}</p>
      <p v-if="msg" class="mt-4 text-sm text-mint-500">{{ msg }}</p>

      <div class="mt-5 grid gap-5 lg:grid-cols-[1fr_22rem]">
        <!-- Reports -->
        <section class="space-y-3">
          <h2 class="text-lg font-bold">{{ $t('codrisk.admin.customer.reports') }}</h2>
          <div v-for="r in doc.reports" :key="r.id" class="card p-4">
            <div class="flex flex-wrap items-start justify-between gap-2">
              <div>
                <div class="font-semibold">{{ $t(`codrisk.reason.${r.reason}`) }} · {{ money(r.amount_cents, r.currency) }}</div>
                <div class="text-xs text-muted">{{ r.shop_name }} · <span class="font-mono">{{ r.order_number }}</span> · {{ r.carrier_code ? carrierName(r.carrier_code) : '—' }} <span class="font-mono">{{ r.tracking_no }}</span></div>
                <div class="text-xs text-muted">{{ fmtDateTime(r.created_at) }}<span v-if="r.block_height" class="font-mono"> · #{{ r.block_height }}</span></div>
              </div>
              <UBadge :color="reportColor(r.status)" variant="soft">{{ $t(`codrisk.status.${r.status}`) }}</UBadge>
            </div>
            <div v-if="r.note" class="mt-2 rounded-xl bg-[var(--field)] p-3 text-sm"><span class="text-xs font-semibold text-muted">{{ $t('codrisk.admin.customer.sellerNote') }}:</span> {{ r.note }}</div>
            <div v-if="r.decision_note" class="mt-2 text-xs text-muted">{{ $t('codrisk.seller.adminNote', { note: r.decision_note }) }}</div>

            <template v-if="r.status !== 'withdrawn'">
              <div v-if="open(r)" class="mt-3 space-y-2 border-t border-line pt-3">
                <UTextarea v-model="d(r).note" :rows="2" class="w-full" maxlength="1000" :placeholder="$t('codrisk.admin.customer.decisionNotePh')" :aria-label="$t('codrisk.admin.customer.decisionNote')" />
                <div class="flex flex-wrap items-center gap-2">
                  <select v-model="d(r).level" class="input w-60" :aria-label="$t('codrisk.admin.customer.alsoSetLevel')">
                    <option value="">{{ $t('codrisk.admin.customer.keepLevel') }}</option>
                    <option v-for="l in RISK_LEVELS" :key="l" :value="l">{{ $t('codrisk.admin.customer.alsoSetLevel') }}: {{ $t(`codrisk.level.${l}`) }}</option>
                  </select>
                  <div class="ml-auto flex gap-2">
                    <UButton v-if="r.status !== 'confirmed'" size="sm" color="error" icon="i-lucide-check" :disabled="busy || needsNote(r, 'confirm')" @click="decide(r, 'confirm')">{{ $t('codrisk.admin.customer.confirm') }}</UButton>
                    <UButton v-if="r.status !== 'dismissed'" size="sm" color="neutral" variant="soft" icon="i-lucide-x" :disabled="busy || needsNote(r, 'dismiss')" @click="decide(r, 'dismiss')">{{ $t('codrisk.admin.customer.dismiss') }}</UButton>
                  </div>
                </div>
              </div>
              <UButton v-else size="xs" color="neutral" variant="ghost" class="mt-2" icon="i-lucide-undo-2" @click="reopened = new Set([...reopened, r.id])">{{ $t('codrisk.admin.customer.reverse') }}</UButton>
            </template>
          </div>
        </section>

        <!-- Level -->
        <aside class="space-y-4">
          <form class="card space-y-3 p-4" @submit.prevent="saveLevel">
            <h2 class="font-bold">{{ $t('codrisk.admin.customer.setLevel') }}</h2>
            <div class="rounded-xl bg-[var(--field)] p-3 text-sm">
              <div class="text-xs font-semibold text-muted">{{ $t('codrisk.admin.customer.suggested') }}</div>
              <CodRiskLevelBadge :level="doc.suggested_level" class="mt-1" />
              <div class="mt-1 text-xs text-muted">{{ $t('codrisk.admin.customer.suggestedHelp') }}</div>
            </div>
            <div class="space-y-1.5">
              <label v-for="l in RISK_LEVELS" :key="l" class="flex cursor-pointer items-start gap-2 rounded-xl border px-3 py-2 text-sm" :class="lv.level === l ? 'border-brand-500 bg-brand-50/60' : 'border-line'">
                <input v-model="lv.level" type="radio" name="level" :value="l" class="mt-1 accent-brand-500">
                <span><span class="font-semibold">{{ $t(`codrisk.level.${l}`) }}</span><span class="block text-xs text-muted">{{ $t(`codrisk.levelHelp.${l}`) }}</span></span>
              </label>
            </div>
            <div>
              <label for="lv-note" class="label">{{ $t('codrisk.admin.customer.levelNote') }}</label>
              <UTextarea id="lv-note" v-model="lv.note" :rows="2" class="w-full" maxlength="1000" :placeholder="$t('codrisk.admin.customer.levelNotePh')" />
            </div>
            <div v-if="lv.level !== 'none'">
              <label for="lv-days" class="label">{{ $t('codrisk.admin.customer.expiresDays') }}</label>
              <UInput id="lv-days" v-model.number="lv.days" type="number" min="1" max="3650" class="w-full" :placeholder="$t('codrisk.admin.customer.expiresNever')" />
            </div>
            <UButton type="submit" block icon="i-lucide-shield" :loading="busy" :disabled="busy || (lv.level !== 'none' && !lv.note.trim())">{{ $t('codrisk.admin.customer.save') }}</UButton>
          </form>

          <section class="card p-4">
            <h2 class="font-bold">{{ $t('codrisk.admin.customer.ledger') }}</h2>
            <p class="mt-1 text-xs text-muted">{{ $t('codrisk.admin.customer.ledgerHelp') }}</p>
            <ol class="mt-3 space-y-2 border-l-2 border-line pl-3">
              <li v-for="b in doc.ledger" :key="b.height" class="text-sm">
                <div class="flex items-center justify-between gap-2">
                  <span class="font-semibold">{{ $t(eventKey(b.event)) }}<span v-if="b.event === 'level.set'" class="font-normal text-muted"> → {{ $t(`codrisk.level.${b.payload.level}`) }}</span></span>
                  <button class="font-mono text-[11px] text-brand-700 hover:underline" @click="proofOf = b.height">#{{ b.height }}</button>
                </div>
                <div class="text-[11px] text-muted">{{ fmtDateTime(b.at) }}</div>
                <div class="text-[11px]" :class="b.anchor_tx ? 'text-mint-500' : 'text-muted'">
                  {{ b.anchor_tx ? $t('codrisk.admin.customer.anchoredIn', { tx: shortHash(b.anchor_tx, 12) }) : $t('codrisk.admin.customer.notAnchored') }}
                </div>
              </li>
            </ol>
          </section>
        </aside>
      </div>
    </template>
    <CodRiskProofModal v-if="proofOf" :height="proofOf" @close="proofOf = null" />
  </div>
</template>
