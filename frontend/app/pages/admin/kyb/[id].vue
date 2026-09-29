<script setup lang="ts">
import type { KybDocument, KybView } from '~/utils/types'

definePageMeta({ layout: 'admin', middleware: 'admin' })
const route = useRoute()
const api = useApi()
const { t } = useI18n()
const { data, refresh } = await useAsyncData(`admin-kyb-${route.params.id}`, () => api<KybView>(`/admin/kyb/${route.params.id}`))
const reveal = ref(false)
const mask = (full: string | null | undefined, last4: string) => (reveal.value ? full || '—' : last4 ? `•••• ${last4}` : '—')
const p = computed(() => data.value?.profile)
const rows = computed(() => {
  const x = p.value
  if (!x) return []
  return [
    [t('kyb.company.type'), x.company_type ? t(`kyb.companyType.${x.company_type}`) : '—'],
    [t('kyb.company.legalName'), x.legal_name], [t('kyb.company.legalNameLocal'), x.legal_name_local],
    [t('kyb.company.registrationNo'), x.registration_no], [t('kyb.company.registrationDate'), x.registration_date ? fmtDate(x.registration_date) : ''],
    [t('kyb.company.taxId'), x.tax_id], [t('kyb.company.country'), x.country], [t('kyb.company.province'), x.province],
    [t('kyb.company.address'), x.registered_address], [t('kyb.company.activity'), x.business_activity],
    [t('kyb.company.phone'), x.contact_phone], [t('kyb.company.email'), x.contact_email], [t('kyb.company.website'), x.website],
  ].filter(([, v]) => v)
})
// per-document decisions
const dd = reactive<Record<string, { status: string; note: string }>>({})
watch(data, (v) => {
  for (const d of v?.documents ?? []) dd[d.id] = { status: d.status, note: d.review_note }
}, { immediate: true })
const note = ref('')
const busy = ref(false)
const error = ref('')
const msg = ref('')
async function decide(action: 'approve' | 'request_changes' | 'reject' | 'revoke') {
  busy.value = true
  error.value = ''
  msg.value = ''
  try {
    const documents = (data.value?.documents ?? []).filter((d) => dd[d.id] && (dd[d.id]!.status !== d.status || dd[d.id]!.note !== d.review_note)).map((d) => ({ id: d.id, ...dd[d.id]! }))
    data.value = await api<KybView>(`/admin/kyb/${route.params.id}/decision`, { method: 'POST', body: { action, note: note.value, documents } })
    note.value = ''
    msg.value = t(`kyb.admin.done.${action}`)
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
const viewing = ref<KybDocument | null>(null)
function view(d: KybDocument) {
  viewing.value = d
  setTimeout(() => refresh(), 800) // pick up the audit entry
}
const ubo = computed(() => (data.value?.persons ?? []).filter((x) => x.roles.includes('ubo')).reduce((a, x) => a + x.ownership_bps, 0) / 100)
</script>

<template>
  <div v-if="data" class="grid gap-6 lg:grid-cols-[1fr_340px]">
    <div class="min-w-0 space-y-5">
      <div>
        <NuxtLink to="/admin/kyb" class="text-sm text-muted hover:text-ink">← {{ $t('kyb.admin.title') }}</NuxtLink>
        <div class="mt-2 flex flex-wrap items-center gap-3">
          <h1 class="page-title">{{ p?.legal_name || data.shop.name }}</h1>
          <KybStatusBadge :status="data.shop.kyb_status" :verified="!!data.shop.kyb_verified_at" />
        </div>
        <p class="text-sm text-muted">{{ data.shop.name }} · <NuxtLink :to="`/s/${data.shop.slug}`" target="_blank" class="underline">/s/{{ data.shop.slug }}</NuxtLink> · {{ data.owner?.email || data.owner?.phone }}</p>
      </div>
      <div v-if="p?.risk_flags.length" class="flex flex-wrap gap-2">
        <span v-for="fl in p.risk_flags" :key="fl" class="chip bg-sun-400/30 font-semibold text-amber-800">⚠ {{ $t(`kyb.flag.${fl}`) }}</span>
      </div>
      <div v-if="data.missing.length" class="rounded-2xl bg-sun-400/20 p-3 text-sm">{{ $t('kyb.admin.incomplete', { n: data.missing.length }) }}</div>

      <section class="card p-5">
        <div class="flex items-center justify-between"><h2 class="font-bold">{{ $t('kyb.company.title') }}</h2>
          <label class="flex items-center gap-2 text-sm"><input v-model="reveal" type="checkbox" class="size-4 accent-brand-500"> {{ $t('kyb.admin.reveal') }}</label></div>
        <dl class="mt-3 grid gap-x-6 text-sm sm:grid-cols-2">
          <div v-for="[k, v] in rows" :key="k" class="flex justify-between gap-4 border-b border-line/70 py-2"><dt class="text-muted">{{ k }}</dt><dd class="text-right font-semibold">{{ v }}</dd></div>
        </dl>
      </section>

      <section class="card p-5">
        <h2 class="font-bold">{{ $t('kyb.people.title') }}</h2>
        <p class="text-xs text-muted">{{ $t('kyb.people.ownershipTotal', { pct: num(ubo, 2) }) }}</p>
        <div class="mt-3 overflow-x-auto">
          <table class="w-full text-sm">
            <thead class="text-left text-xs uppercase tracking-wider text-muted"><tr><th class="py-2 pr-3">{{ $t('kyb.people.fullName') }}</th><th class="py-2 pr-3">{{ $t('kyb.admin.roles') }}</th><th class="py-2 pr-3">{{ $t('kyb.people.idNumber') }}</th><th class="py-2 pr-3">{{ $t('kyb.people.nationality') }}</th><th class="py-2">{{ $t('kyb.people.ownership') }}</th></tr></thead>
            <tbody>
              <tr v-for="x in data.persons" :key="x.id" class="border-t border-line/60">
                <td class="py-2 pr-3 font-semibold">{{ x.full_name }}<div class="text-xs font-normal text-muted">{{ x.title }}<template v-if="x.date_of_birth"> · {{ fmtDate(x.date_of_birth) }}</template></div><span v-if="x.is_pep" class="chip mt-1 bg-sun-400/30 text-amber-800">PEP</span></td>
                <td class="py-2 pr-3">{{ x.roles.map((r) => $t(`kyb.role.${r}`)).join(', ') }}</td>
                <td class="py-2 pr-3 font-mono text-xs">{{ $t(`kyb.idType.${x.id_type}`) }}<br>{{ mask(x.id_number, x.id_last4) }}</td>
                <td class="py-2 pr-3">{{ x.nationality }}</td>
                <td class="py-2">{{ x.roles.includes('ubo') ? `${num(x.ownership_bps / 100, 2)}%` : '—' }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <section class="card p-5">
        <h2 class="font-bold">{{ $t('kyb.bank.title') }}</h2>
        <p class="mt-2 text-sm">{{ p?.bank_name }} · {{ p?.bank_account_name }} · <span class="font-mono">{{ mask(p?.bank_account_number, p?.bank_account_last4 ?? '') }}</span></p>
        <p v-if="p && p.bank_account_name && p.legal_name && !p.bank_account_name.toLowerCase().includes(p.legal_name.toLowerCase().split(/[ ,]/)[0] ?? '')" class="mt-1 text-xs font-semibold text-amber-700">⚠ {{ $t('kyb.admin.nameMismatch') }}</p>
      </section>

      <section class="card p-5">
        <h2 class="font-bold">{{ $t('kyb.docs.title') }}</h2>
        <ul class="mt-3 space-y-3">
          <li v-for="d in data.documents" :key="d.id" class="rounded-xl border border-line p-3 text-sm">
            <div class="flex flex-wrap items-center gap-2">
              <span class="font-semibold">{{ $t(`kyb.doc.${d.kind}`) }}</span>
              <span class="text-xs text-muted">{{ d.original_name }} · {{ fmtDateTime(d.created_at) }}<template v-if="d.expires_on"> · {{ $t('kyb.docs.expires', { date: fmtDate(d.expires_on) }) }}</template></span>
              <UButton color="neutral" variant="soft" size="sm" type="submit" class="ml-auto" @click="view(d)">{{ $t('common.view') }}</UButton>
            </div>
            <div v-if="dd[d.id]" class="mt-2 flex flex-wrap items-center gap-2">
              <button v-for="s in (['accepted', 'rejected', 'pending'] as const)" :key="s" class="chip border px-3 py-1" :class="dd[d.id]!.status === s ? 'border-brand-500 bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]' : 'border-transparent bg-surface shadow-card hover:text-brand-500'" @click="dd[d.id]!.status = s">{{ $t(`kyb.docStatus.${s}`) }}</button>
              <UInput size="md" v-if="dd[d.id]!.status === 'rejected'" v-model="dd[d.id]!.note" class="flex-1" :placeholder="$t('kyb.admin.docReason')" :aria-label="$t('kyb.admin.docReason')" />
            </div>
            <div class="mt-1 font-mono text-[10px] text-muted">sha256 {{ d.sha256.slice(0, 16) }}…</div>
          </li>
        </ul>
      </section>
    </div>

    <aside class="space-y-5 lg:sticky lg:top-20 lg:self-start">
      <section class="card space-y-3 p-5">
        <h2 class="font-bold">{{ $t('kyb.admin.decision') }}</h2>
        <UTextarea v-model="note" :rows="4" maxlength="2000" :placeholder="$t('kyb.admin.notePh')" :aria-label="$t('kyb.admin.notePh')" />
        <template v-if="data.shop.kyb_status === 'submitted'">
          <UButton type="submit" class="w-full" :disabled="busy" @click="decide('approve')">✓ {{ $t('kyb.admin.approve') }}</UButton>
          <UButton color="neutral" variant="soft" type="submit" class="w-full" :disabled="busy" @click="decide('request_changes')">{{ $t('kyb.admin.requestChanges') }}</UButton>
          <UButton color="neutral" variant="soft" type="submit" class="w-full text-brand-700" :disabled="busy" @click="decide('reject')">{{ $t('kyb.admin.reject') }}</UButton>
        </template>
        <p v-else class="text-sm text-muted">{{ $t('kyb.admin.notSubmitted') }}</p>
        <UButton color="neutral" variant="soft" type="submit" v-if="data.shop.kyb_verified_at" class="w-full text-brand-700" :disabled="busy" @click="decide('revoke')">{{ $t('kyb.admin.revoke') }}</UButton>
        <p class="text-xs text-muted">{{ $t('kyb.admin.decisionHint') }}</p>
        <p v-if="msg" class="text-sm text-mint-500" role="status">{{ msg }}</p>
        <p v-if="error" class="text-sm text-brand-700" role="alert">{{ error }}</p>
      </section>
      <section class="card p-5">
        <h2 class="font-bold">{{ $t('kyb.history') }}</h2>
        <ol class="mt-3 max-h-96 space-y-2 overflow-y-auto text-xs">
          <li v-for="(e, i) in data.events" :key="i">
            <span class="text-muted">{{ fmtDateTime(e.created_at) }}</span> · <b>{{ $te(`kyb.event.${e.action}`) ? $t(`kyb.event.${e.action}`) : e.action }}</b>
            <span v-if="e.actor"> · {{ e.actor }}</span>
            <span v-if="e.note" class="block text-muted">{{ $te(`kyb.doc.${e.note}`) ? $t(`kyb.doc.${e.note}`) : e.note }}</span>
          </li>
        </ol>
      </section>
    </aside>
    <KybDocViewer v-if="viewing" :doc-id="viewing.id" :title="$t(`kyb.doc.${viewing.kind}`)" @close="viewing = null" />
  </div>
</template>
