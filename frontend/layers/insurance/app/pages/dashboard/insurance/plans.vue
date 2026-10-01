<script setup lang="ts">
/** Agent: insurance plans offered on zaokaiy (fixed premium, or a rate of the sum insured). */
import { PLAN_KINDS, planName, type Plan } from '#insurance/utils/insurance'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId, shop } = useShop()
const { locale } = useI18n()
const cur = computed(() => shop.value?.currency ?? 'LAK')
const { data: plans, refresh } = await useAsyncData('agent-plans', () => api<Plan[]>(`/shops/${shopId.value}/insurance/plans`), { watch: [shopId], default: () => [] })

const editing = ref<string | null>(null)
const blank = () => ({ kind: 'motor', insurer: '', name: '', name_lo: '', description: '', coverage: '', premium_mode: 'fixed' as 'fixed' | 'rate', premium: '' as number | string,
  rate_pct: '' as number | string, min_premium: '' as number | string, sum_min: '' as number | string, sum_max: '' as number | string, term_months: 12 as number | string, active: true })
const f = reactive(blank())
const toMajor = (c: number) => (c ? c / 100 : '')
function open(p?: Plan) {
  editing.value = p?.id ?? 'new'
  Object.assign(f, p
    ? { kind: p.kind, insurer: p.insurer, name: p.name, name_lo: p.name_lo, description: p.description, coverage: p.coverage.join('\n'), premium_mode: p.premium_mode,
        premium: toMajor(p.premium_cents), rate_pct: p.rate_bps ? p.rate_bps / 100 : '', min_premium: toMajor(p.min_premium_cents), sum_min: toMajor(p.sum_insured_min_cents),
        sum_max: toMajor(p.sum_insured_max_cents), term_months: p.term_months, active: p.active }
    : blank())
}
const cents = (v: number | string) => Math.round(Number(v || 0) * 100)
const error = ref('')
async function save() {
  error.value = ''
  const body = {
    kind: f.kind, insurer: f.insurer, name: f.name, name_lo: f.name_lo, description: f.description,
    coverage: f.coverage.split('\n').map((s) => s.trim()).filter(Boolean), premium_mode: f.premium_mode,
    premium_cents: cents(f.premium), rate_bps: Math.round(Number(f.rate_pct || 0) * 100), min_premium_cents: cents(f.min_premium),
    sum_insured_min_cents: cents(f.sum_min), sum_insured_max_cents: cents(f.sum_max), term_months: Number(f.term_months || 12), active: f.active,
  }
  try {
    if (editing.value === 'new') await api(`/shops/${shopId.value}/insurance/plans`, { method: 'POST', body })
    else await api(`/insurance/plans/${editing.value}`, { method: 'PATCH', body })
    editing.value = null
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
async function toggle(p: Plan) {
  await api(`/insurance/plans/${p.id}`, { method: 'PATCH', body: { active: !p.active } })
  await refresh()
}
const modal = computed({ get: () => !!editing.value, set: (v) => !v && (editing.value = null) })
</script>

<template>
  <div>
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 class="page-title">{{ $t('insurance.agent.plansTitle') }}</h1>
        <p class="text-sm text-muted">{{ $t('insurance.agent.plansSubtitle') }}</p>
      </div>
      <UButton icon="i-lucide-plus" @click="open()">{{ $t('insurance.agent.newPlan') }}</UButton>
    </div>
    <EmptyState v-if="!plans.length" class="mt-6" :title="$t('insurance.agent.plansEmptyTitle')" :text="$t('insurance.agent.plansEmptyText')" />
    <div class="mt-6 grid gap-3 md:grid-cols-2">
      <article v-for="p in plans" :key="p.id" class="card flex items-start gap-3 p-4" :class="!p.active && 'opacity-60'">
        <button type="button" class="min-w-0 flex-1 text-left" @click="open(p)">
          <div class="text-xs font-bold uppercase tracking-wider text-muted">{{ $t(`insurance.kind.${p.kind}`) }} · {{ p.insurer }}</div>
          <div class="font-bold">{{ planName(p, locale) }}</div>
          <div class="text-sm text-muted">{{ p.premium_mode === 'rate' ? $t('insurance.plan.rateOf', { pct: pct(p.rate_bps) }) : money(p.premium_cents, cur) }} · {{ $t('insurance.plan.months', { n: p.term_months }) }}</div>
        </button>
        <USwitch :model-value="p.active" :label="$t('insurance.agent.active')" @update:model-value="toggle(p)" />
      </article>
    </div>

    <UModal v-model:open="modal" :title="editing === 'new' ? $t('insurance.agent.newPlan') : $t('insurance.agent.editPlan')" :ui="{ content: 'max-w-2xl' }">
      <template #body>
        <form class="space-y-3" @submit.prevent="save">
          <div class="grid gap-3 sm:grid-cols-2">
            <div>
              <label class="label" for="pl-kind">{{ $t('insurance.agent.kind') }}</label>
              <select id="pl-kind" v-model="f.kind" class="input"><option v-for="k in PLAN_KINDS" :key="k" :value="k">{{ $t(`insurance.kind.${k}`) }}</option></select>
            </div>
            <div><label class="label" for="pl-ins">{{ $t('insurance.agent.insurer') }}</label><UInput id="pl-ins" v-model="f.insurer" required maxlength="120" /></div>
            <div><label class="label" for="pl-name">{{ $t('common.name') }}</label><UInput id="pl-name" v-model="f.name" required maxlength="120" /></div>
            <div><label class="label" for="pl-lo">{{ $t('insurance.agent.nameLo') }}</label><UInput id="pl-lo" v-model="f.name_lo" maxlength="120" /></div>
          </div>
          <div><label class="label" for="pl-desc">{{ $t('common.description') }}</label><UTextarea id="pl-desc" v-model="f.description" :rows="2" /></div>
          <div><label class="label" for="pl-cov">{{ $t('insurance.agent.coverage') }}</label><UTextarea id="pl-cov" v-model="f.coverage" :rows="3" :placeholder="$t('insurance.agent.coverageHint')" /></div>
          <div class="grid grid-cols-2 gap-2" role="radiogroup">
            <button v-for="m in (['fixed', 'rate'] as const)" :key="m" type="button" role="radio" :aria-checked="f.premium_mode === m" class="rounded-xl border-2 p-2 text-sm font-semibold" :class="f.premium_mode === m ? 'border-brand-500 bg-brand-500/10 text-brand-600' : 'border-line'" @click="f.premium_mode = m">{{ $t(`insurance.agent.mode.${m}`) }}</button>
          </div>
          <div class="grid gap-3 sm:grid-cols-3">
            <div v-if="f.premium_mode === 'fixed'"><label class="label" for="pl-prem">{{ $t('insurance.agent.premium', { cur }) }}</label><UInput id="pl-prem" v-model.number="f.premium" type="number" min="0" step="any" /></div>
            <div v-else><label class="label" for="pl-rate">{{ $t('insurance.agent.rate') }}</label><UInput id="pl-rate" v-model.number="f.rate_pct" type="number" min="0" max="100" step="0.01" /></div>
            <div><label class="label" for="pl-min">{{ $t('insurance.agent.minPremium', { cur }) }}</label><UInput id="pl-min" v-model.number="f.min_premium" type="number" min="0" step="any" /></div>
            <div><label class="label" for="pl-term">{{ $t('insurance.agent.term') }}</label><UInput id="pl-term" v-model.number="f.term_months" type="number" min="1" max="120" /></div>
            <div><label class="label" for="pl-smin">{{ $t('insurance.agent.sumMin', { cur }) }}</label><UInput id="pl-smin" v-model.number="f.sum_min" type="number" min="0" step="any" /></div>
            <div><label class="label" for="pl-smax">{{ $t('insurance.agent.sumMax', { cur }) }}</label><UInput id="pl-smax" v-model.number="f.sum_max" type="number" min="0" step="any" /></div>
          </div>
          <USwitch v-model="f.active" :label="$t('insurance.agent.active')" />
          <p v-if="error" class="text-sm text-brand-700" role="alert">{{ error }}</p>
          <UButton type="submit" block>{{ $t('common.save') }}</UButton>
        </form>
      </template>
    </UModal>
  </div>
</template>
