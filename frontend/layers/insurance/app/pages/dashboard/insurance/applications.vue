<script setup lang="ts">
/** Agent: applications → review, collect premium, issue policy (or reject / cancel with a note). */
import type { Application } from '#insurance/utils/insurance'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId } = useShop()
const status = ref('')
const { data: apps, refresh } = await useAsyncData('agent-apps', () => api<Application[]>(`/shops/${shopId.value}/insurance/applications`, { query: { status: status.value || undefined } }), {
  watch: [shopId, status],
  default: () => [],
})
const STATUSES = ['', 'submitted', 'reviewing', 'approved', 'issued', 'rejected', 'cancelled']
const inputs = reactive<Record<string, { policy_no: string; note: string }>>({})
const box = (id: string) => (inputs[id] ??= { policy_no: '', note: '' })
const error = ref<Record<string, string>>({})
async function act(a: Application, action: string) {
  error.value[a.id] = ''
  try {
    await api(`/insurance/applications/${a.id}/decision`, { method: 'POST', body: { action, ...box(a.id) } })
    await refresh()
  } catch (e) {
    error.value[a.id] = apiError(e)
  }
}
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('insurance.agent.appsTitle') }}</h1>
    <p class="text-sm text-muted">{{ $t('insurance.agent.appsSubtitle') }}</p>
    <div class="mt-4 flex flex-wrap gap-2">
      <UButton v-for="s in STATUSES" :key="s" size="sm" :color="status === s ? 'primary' : 'neutral'" :variant="status === s ? 'solid' : 'soft'" @click="status = s">{{ s ? $t(`insurance.status.${s}`) : $t('common.all') }}</UButton>
    </div>
    <EmptyState v-if="!apps.length" class="mt-6" :title="$t('insurance.agent.appsEmptyTitle')" :text="$t('insurance.agent.appsEmptyText')" />
    <div class="mt-4 space-y-3">
      <article v-for="a in apps" :key="a.id" class="card p-5" :data-testid="`app-${a.number}`">
        <div class="flex flex-wrap items-start justify-between gap-2">
          <div>
            <div class="text-xs text-muted">{{ a.number }} · {{ fmtDateTime(a.created_at) }}</div>
            <h2 class="font-bold">{{ a.applicant_name }} <a :href="telLink(a.phone)" class="ml-1 text-sm font-semibold text-brand-600">{{ a.phone }}</a></h2>
            <div class="text-sm text-muted">{{ $t(`insurance.kind.${a.kind}`) }} · {{ a.plan_name }} · {{ a.insurer }}</div>
          </div>
          <div class="flex items-center gap-2">
            <UBadge :color="a.paid ? 'success' : 'neutral'" :label="a.paid ? $t('insurance.my.paid') : $t('insurance.my.unpaid')" />
            <InsuranceStatusBadge :status="a.status" />
          </div>
        </div>
        <div class="mt-3 grid grid-cols-2 gap-2 text-sm sm:grid-cols-4">
          <div><div class="text-xs text-muted">{{ $t('insurance.apply.quote') }}</div><b>{{ money(a.premium_cents, a.currency) }}</b></div>
          <div v-if="a.sum_insured_cents"><div class="text-xs text-muted">{{ $t('insurance.agent.sumInsured') }}</div><b>{{ money(a.sum_insured_cents, a.currency) }}</b></div>
          <div><div class="text-xs text-muted">{{ $t('insurance.my.period') }}</div><b>{{ fmtDate(a.start_date, 'short') }} – {{ fmtDate(a.end_date, 'short') }}</b></div>
          <div v-if="a.policy_no"><div class="text-xs text-muted">{{ $t('insurance.my.policyNo') }}</div><b>{{ a.policy_no }}</b></div>
        </div>
        <dl v-if="Object.keys(a.details).length" class="mt-3 flex flex-wrap gap-x-5 gap-y-1 rounded-xl bg-paper p-3 text-sm">
          <div v-for="(v, k) in a.details" :key="k"><dt class="inline text-muted">{{ $te(`insurance.details.${k}`) ? $t(`insurance.details.${k}`) : k }}:</dt> <dd class="inline font-semibold">{{ v }}</dd></div>
        </dl>
        <template v-if="!['rejected', 'cancelled'].includes(a.status)">
          <div class="mt-3 grid gap-2 sm:grid-cols-2">
            <UInput v-if="a.status === 'approved'" v-model="box(a.id).policy_no" size="md" :placeholder="$t('insurance.agent.policyNo')" :aria-label="$t('insurance.agent.policyNo')" />
            <UInput v-model="box(a.id).note" size="md" :placeholder="$t('insurance.agent.note')" :aria-label="$t('insurance.agent.note')" />
          </div>
          <div class="mt-3 flex flex-wrap gap-2">
            <UButton v-if="a.status === 'submitted'" size="sm" color="neutral" variant="soft" @click="act(a, 'review')">{{ $t('insurance.agent.review') }}</UButton>
            <UButton v-if="['submitted', 'reviewing'].includes(a.status)" size="sm" @click="act(a, 'approve')">{{ $t('insurance.agent.approve') }}</UButton>
            <UButton size="sm" color="success" variant="soft" @click="act(a, a.paid ? 'unpaid' : 'paid')">{{ a.paid ? $t('insurance.agent.markUnpaid') : $t('insurance.agent.markPaid') }}</UButton>
            <UButton v-if="a.status === 'approved'" size="sm" @click="act(a, 'issue')">{{ $t('insurance.agent.issue') }}</UButton>
            <UButton v-if="a.status !== 'issued'" size="sm" color="error" variant="soft" @click="act(a, 'reject')">{{ $t('insurance.agent.reject') }}</UButton>
            <UButton size="sm" color="neutral" variant="ghost" @click="act(a, 'cancel')">{{ $t('insurance.agent.cancel') }}</UButton>
          </div>
        </template>
        <p v-if="a.decision_note" class="mt-3 text-sm text-muted">{{ a.decision_note }}</p>
        <p v-if="error[a.id]" class="mt-2 text-sm text-brand-700" role="alert">{{ error[a.id] }}</p>
      </article>
    </div>
  </div>
</template>
