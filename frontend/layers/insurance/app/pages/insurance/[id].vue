<script setup lang="ts">
/** Plan page: coverage, instant quote and the online application. */
import { estimate, KIND_ICON, planName, type Application, type Plan } from '#insurance/utils/insurance'

const route = useRoute()
const api = useApi()
const { locale } = useI18n()
const { data: plan, error } = await useAsyncData(`plan:${route.params.id}`, () => api<Plan>(`/insurance/plans/${route.params.id}`))
if (error.value || !plan.value) throw createError({ statusCode: 404, statusMessage: tr('insurance.public.notFound'), fatal: true })
const p = computed(() => plan.value!)
const cur = computed(() => p.value.currency ?? 'LAK')

const sum = ref<number | string>(p.value.sum_insured_min_cents ? p.value.sum_insured_min_cents / 100 : '')
const sumCents = computed(() => Math.round(Number(sum.value || 0) * 100))
const needsSum = computed(() => p.value.premium_mode === 'rate' || p.value.sum_insured_max_cents > 0)
const inRange = computed(() => !p.value.sum_insured_max_cents || (sumCents.value >= p.value.sum_insured_min_cents && sumCents.value <= p.value.sum_insured_max_cents))
const premium = computed(() => (inRange.value ? estimate(p.value, sumCents.value) : null))

const tomorrow = new Date(Date.now() + 86_400_000).toISOString().slice(0, 10)
const form = reactive({ applicant_name: '', phone: '', email: '', start_date: tomorrow })
const DETAIL_FIELDS: Record<string, string[]> = { motor: ['plate', 'make', 'model', 'year'], travel: ['destination', 'return_date'], property: ['address'] }
const fields = computed(() => DETAIL_FIELDS[p.value.kind] ?? [])
const details = reactive<Record<string, string>>({})
const error2 = ref('')
const sending = ref(false)
const done = ref<Application | null>(null)
async function apply() {
  error2.value = ''
  sending.value = true
  try {
    const clean = Object.fromEntries(Object.entries(details).filter(([, v]) => String(v).trim()))
    done.value = await api<Application>(`/insurance/plans/${p.value.id}/apply`, {
      method: 'POST',
      body: { ...form, sum_insured_cents: needsSum.value ? sumCents.value : 0, details: clean },
    })
  } catch (e) {
    error2.value = apiError(e)
  } finally {
    sending.value = false
  }
}
const { loggedIn } = useAuth()
useHead({ title: () => `${planName(p.value, locale.value)} · zaokaiy` })
</script>

<template>
  <div class="mx-auto grid max-w-6xl gap-8 px-4 pt-10 lg:grid-cols-[1fr_420px]">
    <div class="min-w-0 space-y-5">
      <NuxtLink to="/insurance" class="text-sm font-semibold text-muted hover:text-ink">← {{ $t('insurance.public.back') }}</NuxtLink>
      <div class="card p-6">
        <div class="flex items-start gap-4">
          <span class="icon-tile bg-violet-500/12 text-violet-600"><UIcon :name="`i-lucide-${KIND_ICON[p.kind] ?? 'shield'}`" class="size-6" /></span>
          <div class="min-w-0">
            <div class="text-xs font-bold uppercase tracking-wider text-muted">{{ $t(`insurance.kind.${p.kind}`) }} · {{ $t('insurance.plan.by', { insurer: p.insurer }) }}</div>
            <h1 class="text-2xl font-extrabold tracking-tight">{{ planName(p, locale) }}</h1>
            <div class="mt-1 flex flex-wrap items-center gap-2 text-sm text-muted">
              {{ $t('insurance.plan.soldBy') }} <NuxtLink :to="`/s/${p.shop_slug}`" class="font-semibold hover:text-ink">{{ p.shop_name }}</NuxtLink><KybVerifiedBadge v-if="p.shop_verified" />
            </div>
          </div>
        </div>
        <p v-if="p.description" class="mt-4 whitespace-pre-line text-sm text-ink/80">{{ p.description }}</p>
        <h2 v-if="p.coverage.length" class="mt-5 font-bold">{{ $t('insurance.plan.coverage') }}</h2>
        <ul class="mt-2 space-y-1.5 text-sm">
          <li v-for="c in p.coverage" :key="c" class="flex gap-2"><UIcon name="i-lucide-check" class="mt-0.5 size-4 shrink-0 text-mint-500" />{{ c }}</li>
        </ul>
        <div class="mt-5 grid grid-cols-2 gap-2 text-sm sm:grid-cols-3">
          <div class="rounded-xl bg-paper p-3"><div class="text-xs text-muted">{{ $t('insurance.plan.term') }}</div><b>{{ $t('insurance.plan.months', { n: p.term_months }) }}</b></div>
          <div v-if="p.sum_insured_max_cents" class="rounded-xl bg-paper p-3"><div class="text-xs text-muted">{{ $t('insurance.plan.sumRange') }}</div><b>{{ money(p.sum_insured_min_cents, cur) }} – {{ money(p.sum_insured_max_cents, cur) }}</b></div>
          <div class="rounded-xl bg-paper p-3"><div class="text-xs text-muted">{{ $t('insurance.plan.premium') }}</div><b>{{ p.premium_mode === 'rate' ? $t('insurance.plan.rateOf', { pct: pct(p.rate_bps) }) : money(p.premium_cents, cur) }}</b></div>
        </div>
      </div>
      <p class="text-xs text-muted">{{ $t('insurance.public.disclaimer') }}</p>
    </div>

    <div class="space-y-4 lg:sticky lg:top-24 lg:self-start">
      <div v-if="done" class="card p-6 text-center" role="status">
        <UIcon name="i-lucide-circle-check" class="size-10 text-mint-500" />
        <h2 class="mt-2 text-xl font-extrabold">{{ $t('insurance.apply.sentTitle') }}</h2>
        <p class="mt-1 text-sm text-muted">{{ $t('insurance.apply.sentText', { shop: p.shop_name, number: done.number }) }}</p>
        <div class="mt-3 text-2xl font-extrabold" data-testid="app-number">{{ done.number }}</div>
        <div class="mt-1 text-sm">{{ $t('insurance.apply.premiumDue', { amount: money(done.premium_cents, done.currency) }) }}</div>
        <UButton v-if="loggedIn" class="mt-4" color="neutral" variant="soft" to="/insurance/my">{{ $t('insurance.my.title') }}</UButton>
      </div>
      <form v-else class="card space-y-4 p-6" @submit.prevent="apply">
        <h2 class="text-lg font-bold">{{ $t('insurance.apply.title') }}</h2>
        <div v-if="needsSum">
          <label class="label" for="ins-sum">{{ $t('insurance.apply.sumInsured', { cur }) }}</label>
          <UInput id="ins-sum" v-model.number="sum" type="number" min="0" step="any" required />
          <p v-if="!inRange" class="mt-1 text-xs text-brand-700">{{ $t('insurance.apply.range', { min: money(p.sum_insured_min_cents, cur), max: money(p.sum_insured_max_cents, cur) }) }}</p>
        </div>
        <div class="rounded-2xl bg-brand-500/10 p-4">
          <div class="text-xs font-semibold text-muted">{{ $t('insurance.apply.quote') }}</div>
          <div class="text-2xl font-extrabold" data-testid="quote">{{ premium != null ? money(premium, cur) : '—' }}</div>
          <div class="text-xs text-muted">{{ $t('insurance.plan.months', { n: p.term_months }) }}</div>
        </div>
        <div><label class="label" for="ins-name">{{ $t('insurance.apply.name') }}</label><UInput id="ins-name" v-model="form.applicant_name" required maxlength="120" /></div>
        <div class="grid gap-3 sm:grid-cols-2">
          <div><label class="label" for="ins-phone">{{ $t('insurance.apply.phone') }}</label><UInput id="ins-phone" v-model="form.phone" type="tel" required placeholder="020 5555 1234" /></div>
          <div><label class="label" for="ins-email">{{ $t('insurance.apply.email') }}</label><UInput id="ins-email" v-model="form.email" type="email" /></div>
        </div>
        <div><label class="label" for="ins-start">{{ $t('insurance.apply.start') }}</label><UInput id="ins-start" v-model="form.start_date" type="date" :min="tomorrow" required /></div>
        <div v-if="fields.length" class="grid gap-3 sm:grid-cols-2">
          <div v-for="f in fields" :key="f"><label class="label" :for="`ins-${f}`">{{ $t(`insurance.details.${f}`) }}</label><UInput :id="`ins-${f}`" v-model="details[f]" :type="f.endsWith('date') ? 'date' : 'text'" maxlength="120" /></div>
        </div>
        <p v-if="error2" class="text-sm text-brand-700" role="alert">{{ error2 }}</p>
        <UButton type="submit" block :loading="sending" :disabled="needsSum && premium == null">{{ $t('insurance.apply.submit') }}</UButton>
        <p class="text-xs text-muted">{{ $t('insurance.apply.privacy') }}</p>
      </form>
    </div>
  </div>
</template>
