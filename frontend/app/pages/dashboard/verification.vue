<script setup lang="ts">
import type { KybDocument, KybView, Shop } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { t } = useI18n()
const { shop, shopId, replaceShop } = useShop()

const { data, refresh } = await useAsyncData('kyb', () => (shopId.value ? api<KybView>(`/shops/${shopId.value}/kyb`) : Promise.resolve(null)), {
  watch: [shopId],
})
const isBusiness = computed(() => shop.value?.entity_type === 'business')
const locked = computed(() => !!data.value && !data.value.editable)
const status = computed(() => data.value?.shop.kyb_status ?? 'none')

// ---- form state (numbers are write-only: blank = keep the stored one)
interface PersonDraft { id?: string; roles: string[]; full_name: string; title: string; nationality: string; date_of_birth: string; id_type: string; id_number: string; id_last4: string; ownership: number | string; is_pep: boolean }
const f = reactive({
  company_type: '', legal_name: '', legal_name_local: '', registration_no: '', registration_date: '', tax_id: '', country: 'LA',
  province: '', registered_address: '', business_activity: '', website: '', contact_email: '', contact_phone: '',
  bank_name: '', bank_account_name: '', bank_account_number: '', bank_last4: '',
})
const persons = ref<PersonDraft[]>([])
function fill(v: KybView | null | undefined) {
  if (!v) return
  const p = v.profile
  Object.assign(f, {
    company_type: p?.company_type ?? '', legal_name: p?.legal_name ?? shop.value?.legal_name ?? '', legal_name_local: p?.legal_name_local ?? '',
    registration_no: p?.registration_no ?? '', registration_date: p?.registration_date ?? '', tax_id: p?.tax_id ?? shop.value?.tax_id ?? '',
    country: p?.country ?? 'LA', province: p?.province ?? '', registered_address: p?.registered_address ?? shop.value?.address ?? '',
    business_activity: p?.business_activity ?? '', website: p?.website ?? '', contact_email: p?.contact_email ?? '',
    contact_phone: p?.contact_phone ?? shop.value?.phone ?? '', bank_name: p?.bank_name ?? '', bank_account_name: p?.bank_account_name ?? '',
    bank_account_number: '', bank_last4: p?.bank_account_last4 ?? '',
  })
  persons.value = v.persons.length
    ? v.persons.map((x) => ({ id: x.id, roles: [...x.roles], full_name: x.full_name, title: x.title, nationality: x.nationality, date_of_birth: x.date_of_birth ?? '',
        id_type: x.id_type, id_number: '', id_last4: x.id_last4, ownership: x.ownership_bps / 100, is_pep: x.is_pep }))
    : [blankPerson(['representative', 'director'])]
}
function blankPerson(roles: string[] = ['ubo']): PersonDraft {
  return { roles, full_name: '', title: '', nationality: f.country || 'LA', date_of_birth: '', id_type: 'national_id', id_number: '', id_last4: '', ownership: '', is_pep: false }
}
watch(data, fill, { immediate: true })

const shareholders = computed(() => data.value?.options.shareholder_types.includes(f.company_type) ?? false)
const rep = computed(() => persons.value.find((p) => p.roles.includes('representative')))
const required = computed(() => {
  const r = ['registration_certificate', 'tax_certificate', 'representative_id']
  if (shareholders.value) r.push('shareholder_register')
  if (rep.value && !rep.value.roles.includes('director')) r.push('authorization_letter')
  return r
})
const docKinds = computed(() => {
  const all = data.value?.options.doc_kinds ?? []
  return [...required.value, ...all.filter((k) => !required.value.includes(k))]
})
const docsBy = (k: string) => (data.value?.documents ?? []).filter((d) => d.kind === k)
const ownershipTotal = computed(() => persons.value.filter((p) => p.roles.includes('ubo')).reduce((a, p) => a + (Number(p.ownership) || 0), 0))
const missingLabels = computed(() =>
  (data.value?.missing ?? []).map((m) => (m.startsWith('doc:') ? t('kyb.missing.doc', { doc: t(`kyb.doc.${m.slice(4)}`) }) : t(`kyb.missing.${m}`))),
)
const progress = computed(() => {
  const total = 10 + 3 + required.value.length
  return Math.max(5, Math.round(((total - (data.value?.missing.length ?? total)) / total) * 100))
})

const busy = ref(false)
const msg = ref('')
const error = ref('')
function body() {
  return {
    ...f,
    registration_date: f.registration_date || null,
    bank_account_number: f.bank_account_number.trim() ? f.bank_account_number : undefined,
    bank_last4: undefined,
    persons: persons.value.map((p) => ({
      id: p.id, roles: p.roles, full_name: p.full_name, title: p.title, nationality: p.nationality, date_of_birth: p.date_of_birth || null,
      id_type: p.id_type, id_number: p.id_number.trim() ? p.id_number : undefined,
      ownership_bps: p.roles.includes('ubo') ? Math.round((Number(p.ownership) || 0) * 100) : 0, is_pep: p.is_pep,
    })),
  }
}
async function save(quiet = false) {
  busy.value = true
  error.value = ''
  msg.value = ''
  try {
    data.value = await api<KybView>(`/shops/${shopId.value}/kyb`, { method: 'PUT', body: body() })
    if (!quiet) msg.value = t('common.saved')
    return true
  } catch (e) {
    error.value = apiError(e)
    return false
  } finally {
    busy.value = false
  }
}
async function submit() {
  if (!(await save(true))) return
  busy.value = true
  try {
    data.value = await api<KybView>(`/shops/${shopId.value}/kyb/submit`, { method: 'POST' })
    msg.value = t('kyb.submitted')
    document.querySelector('h1')?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
async function becomeBusiness() {
  error.value = ''
  try {
    const s = await api<Shop>(`/shops/${shopId.value}`, { method: 'PATCH', body: { entity_type: 'business' } })
    replaceShop(s)
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}

// ---- documents
const upl = reactive<Record<string, { expires: string; busy: boolean; error: string }>>({})
const u = (k: string) => (upl[k] ??= { expires: '', busy: false, error: '' })
async function upload(kind: string, ev: Event) {
  const input = ev.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  const s = u(kind)
  s.error = ''
  if (file.size > (data.value?.options.max_doc_mb ?? 10) * 1048576) {
    s.error = t('kyb.docs.tooBig', { mb: data.value?.options.max_doc_mb ?? 10 })
    return
  }
  s.busy = true
  try {
    const fd = new FormData()
    fd.append('kind', kind)
    if (s.expires) fd.append('expires_on', s.expires)
    fd.append('file', file)
    await api(`/shops/${shopId.value}/kyb/documents`, { method: 'POST', body: fd })
    s.expires = ''
    await refresh()
  } catch (e) {
    s.error = apiError(e)
  } finally {
    s.busy = false
  }
}
async function removeDoc(d: KybDocument) {
  error.value = ''
  try {
    await api(`/kyb/documents/${d.id}`, { method: 'DELETE' })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
const viewing = ref<KybDocument | null>(null)
const kb = (n: number) => (n > 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.ceil(n / 1024)} KB`)
const COUNTRIES = ['LA', 'TH', 'VN', 'CN', 'KH', 'MM', 'KR', 'JP', 'SG', 'MY', 'US', 'FR']
const countryName = (c: string) => (COUNTRIES.includes(c) ? t(`kyb.countries.${c}`) : c)
const docStatusCls: Record<string, string> = { pending: 'bg-line text-ink', accepted: 'bg-mint-500/15 text-mint-500', rejected: 'bg-brand-500 text-white' }
</script>

<template>
  <div class="max-w-4xl">
    <div class="flex flex-wrap items-center gap-3">
      <h1 class="page-title">{{ $t('kyb.title') }}</h1>
      <KybStatusBadge v-if="data && isBusiness" :status="status" :verified="!!data.shop.kyb_verified_at" />
    </div>
    <p class="text-sm text-muted">{{ $t('kyb.subtitle') }}</p>

    <!-- individual shop: explain + switch -->
    <div v-if="shop && !isBusiness" class="card mt-6 space-y-3 p-6">
      <div class="font-bold">{{ $t('kyb.individual.title') }}</div>
      <p class="text-sm text-ink/80">{{ $t('kyb.individual.text') }}</p>
      <UButton type="submit" @click="becomeBusiness">{{ $t('kyb.individual.switch') }}</UButton>
      <p v-if="error" class="text-sm text-brand-700" role="alert">{{ error }}</p>
    </div>

    <template v-else-if="data">
      <!-- status banner -->
      <div v-if="status === 'approved' || data.shop.kyb_verified_at" class="mt-5 flex flex-col items-start gap-2 rounded-2xl bg-mint-500/10 p-4 text-sm sm:flex-row sm:gap-3">
        <KybVerifiedBadge class="shrink-0 whitespace-nowrap" />
        <div><span>{{ $t('kyb.banner.approved', { date: fmtDate(data.shop.kyb_verified_at!) }) }}</span> <span v-if="data.profile?.review_due_at">{{ $t('kyb.banner.reviewDue', { date: fmtDate(data.profile.review_due_at) }) }}</span></div>
      </div>
      <div v-if="status === 'submitted'" class="mt-5 rounded-2xl bg-sky-50 p-4 text-sm text-sky-900" role="status">{{ $t('kyb.banner.submitted') }}</div>
      <div v-if="(status === 'changes_requested' || status === 'rejected' || status === 'revoked') && data.profile?.review_note" class="mt-5 rounded-2xl bg-sun-400/20 p-4 text-sm" role="alert">
        <div class="font-bold">{{ $t(`kyb.banner.${status}`) }}</div>
        <p class="mt-1 whitespace-pre-line">{{ data.profile.review_note }}</p>
      </div>
      <div v-if="data.required_to_publish && !data.shop.kyb_verified_at && status !== 'submitted'" class="mt-5 rounded-2xl bg-paper p-4 text-sm">{{ $t('kyb.banner.gate') }}</div>

      <!-- checklist -->
      <div v-if="!locked && data.missing.length" class="card mt-5 p-5">
        <div class="flex items-center justify-between text-sm font-semibold"><span>{{ $t('kyb.checklist') }}</span><span>{{ progress }}%</span></div>
        <div class="mt-2 h-2 overflow-hidden rounded-full bg-line"><div class="h-full rounded-full bg-brand-500 transition-all" :style="{ width: `${progress}%` }" /></div>
        <ul class="mt-3 grid gap-1 text-sm text-ink/80 sm:grid-cols-2">
          <li v-for="m in missingLabels" :key="m">○ {{ m }}</li>
        </ul>
      </div>

      <form class="mt-5 space-y-5" @submit.prevent="submit">
        <fieldset :disabled="locked" class="space-y-5">
          <!-- 1. company -->
          <section class="card space-y-4 p-6">
            <h2 class="font-bold">1 · {{ $t('kyb.company.title') }}</h2>
            <div class="grid gap-3 sm:grid-cols-2">
              <div class="sm:col-span-2">
                <label for="k-type" class="label">{{ $t('kyb.company.type') }}</label>
                <select id="k-type" v-model="f.company_type" class="input"><option value="" disabled>{{ $t('kyb.select') }}</option><option v-for="c in data.options.company_types" :key="c" :value="c">{{ $t(`kyb.companyType.${c}`) }}</option></select>
              </div>
              <div><label for="k-legal" class="label">{{ $t('kyb.company.legalName') }}</label><UInput id="k-legal" v-model="f.legal_name" maxlength="200" :placeholder="$t('kyb.company.legalNamePh')" /></div>
              <div><label for="k-legal-lo" class="label">{{ $t('kyb.company.legalNameLocal') }}</label><UInput id="k-legal-lo" v-model="f.legal_name_local" maxlength="200" /></div>
              <div><label for="k-reg" class="label">{{ $t('kyb.company.registrationNo') }}</label><UInput id="k-reg" v-model="f.registration_no" maxlength="60" class="font-mono" /></div>
              <div><label for="k-regdate" class="label">{{ $t('kyb.company.registrationDate') }}</label><UInput id="k-regdate" v-model="f.registration_date" type="date" /></div>
              <div><label for="k-tax" class="label">{{ $t('kyb.company.taxId') }}</label><UInput id="k-tax" v-model="f.tax_id" maxlength="40" class="font-mono" /></div>
              <div>
                <label for="k-country" class="label">{{ $t('kyb.company.country') }}</label>
                <select id="k-country" v-model="f.country" class="input"><option v-for="c in COUNTRIES" :key="c" :value="c">{{ countryName(c) }}</option></select>
              </div>
              <div><label for="k-prov" class="label">{{ $t('kyb.company.province') }}</label><UInput id="k-prov" v-model="f.province" list="k-provinces" maxlength="80" /><datalist id="k-provinces"><option v-for="p in LAO_PROVINCES" :key="p" :value="p" /></datalist></div>
              <div><label for="k-phone" class="label">{{ $t('kyb.company.phone') }}</label><UInput id="k-phone" v-model="f.contact_phone" type="tel" maxlength="40" /></div>
              <div class="sm:col-span-2"><label for="k-addr" class="label">{{ $t('kyb.company.address') }}</label><UTextarea id="k-addr" v-model="f.registered_address" :rows="2" maxlength="500" /></div>
              <div class="sm:col-span-2"><label for="k-act" class="label">{{ $t('kyb.company.activity') }}</label><UInput id="k-act" v-model="f.business_activity" maxlength="300" :placeholder="$t('kyb.company.activityPh')" /></div>
              <div><label for="k-email" class="label">{{ $t('kyb.company.email') }}</label><UInput id="k-email" v-model="f.contact_email" type="email" maxlength="120" /></div>
              <div><label for="k-web" class="label">{{ $t('kyb.company.website') }} <span class="normal-case tracking-normal text-muted">({{ $t('common.optional') }})</span></label><UInput id="k-web" v-model="f.website" type="url" maxlength="200" placeholder="https://" /></div>
            </div>
          </section>

          <!-- 2. people -->
          <section class="card space-y-4 p-6">
            <div>
              <h2 class="font-bold">2 · {{ $t('kyb.people.title') }}</h2>
              <p class="text-sm text-muted">{{ shareholders ? $t('kyb.people.hintShareholders') : $t('kyb.people.hint') }}</p>
            </div>
            <div v-for="(p, i) in persons" :key="p.id ?? `new-${i}`" class="rounded-2xl border border-line p-4">
              <div class="flex flex-wrap items-center gap-3">
                <span class="font-semibold">{{ p.full_name || $t('kyb.people.person', { n: i + 1 }) }}</span>
                <label v-for="r in data.options.roles" :key="r" class="flex items-center gap-1.5 text-sm">
                  <input v-model="p.roles" type="checkbox" :value="r" class="size-4 accent-brand-500"> {{ $t(`kyb.role.${r}`) }}
                </label>
                <button v-if="persons.length > 1" type="button" class="ml-auto text-sm font-semibold text-brand-600 hover:underline" @click="persons.splice(i, 1)">{{ $t('common.remove') }}</button>
              </div>
              <div class="mt-3 grid gap-3 sm:grid-cols-3">
                <div class="sm:col-span-2"><label :for="`p-name-${i}`" class="label">{{ $t('kyb.people.fullName') }}</label><UInput :id="`p-name-${i}`" v-model="p.full_name" maxlength="120" autocomplete="off" /></div>
                <div><label :for="`p-title-${i}`" class="label">{{ $t('kyb.people.position') }}</label><UInput :id="`p-title-${i}`" v-model="p.title" maxlength="80" /></div>
                <div>
                  <label :for="`p-nat-${i}`" class="label">{{ $t('kyb.people.nationality') }}</label>
                  <select :id="`p-nat-${i}`" v-model="p.nationality" class="input"><option v-for="c in COUNTRIES" :key="c" :value="c">{{ countryName(c) }}</option></select>
                </div>
                <div><label :for="`p-dob-${i}`" class="label">{{ $t('kyb.people.dob') }}</label><UInput :id="`p-dob-${i}`" v-model="p.date_of_birth" type="date" /></div>
                <div>
                  <label :for="`p-idt-${i}`" class="label">{{ $t('kyb.people.idType') }}</label>
                  <select :id="`p-idt-${i}`" v-model="p.id_type" class="input"><option v-for="x in data.options.id_types" :key="x" :value="x">{{ $t(`kyb.idType.${x}`) }}</option></select>
                </div>
                <div class="sm:col-span-2">
                  <label :for="`p-idn-${i}`" class="label">{{ $t('kyb.people.idNumber') }}</label>
                  <UInput :id="`p-idn-${i}`" v-model="p.id_number" maxlength="40" class="font-mono" autocomplete="off" :placeholder="p.id_last4 ? $t('kyb.savedKeep', { last4: p.id_last4 }) : ''" />
                </div>
                <div v-if="p.roles.includes('ubo')">
                  <label :for="`p-own-${i}`" class="label">{{ $t('kyb.people.ownership') }}</label>
                  <UInput :id="`p-own-${i}`" v-model.number="p.ownership" type="number" min="0" max="100" step="0.01" />
                </div>
              </div>
              <label class="mt-3 flex items-start gap-2 text-sm"><input v-model="p.is_pep" type="checkbox" class="mt-0.5 size-4 accent-brand-500"> <span>{{ $t('kyb.people.pep') }}<br><span class="text-xs text-muted">{{ $t('kyb.people.pepHint') }}</span></span></label>
            </div>
            <div class="flex flex-wrap items-center gap-3">
              <UButton color="neutral" variant="soft" size="sm" type="button" @click="persons.push(blankPerson())">+ {{ $t('kyb.people.add') }}</UButton>
              <span v-if="shareholders" class="text-sm" :class="ownershipTotal > 100 ? 'font-semibold text-brand-700' : 'text-muted'">{{ $t('kyb.people.ownershipTotal', { pct: num(ownershipTotal, 2) }) }}</span>
            </div>
            <p class="text-xs text-muted">🔒 {{ $t('kyb.privacy') }}</p>
          </section>

          <!-- 3. bank -->
          <section class="card space-y-4 p-6">
            <div>
              <h2 class="font-bold">3 · {{ $t('kyb.bank.title') }}</h2>
              <p class="text-sm text-muted">{{ $t('kyb.bank.hint') }}</p>
            </div>
            <div class="grid gap-3 sm:grid-cols-3">
              <div><label for="b-name" class="label">{{ $t('kyb.bank.bank') }}</label><UInput id="b-name" v-model="f.bank_name" list="k-banks" maxlength="80" /><datalist id="k-banks"><option v-for="b in ['BCEL', 'LDB', 'APB', 'JDB', 'Indochina Bank', 'ST Bank', 'Maruhan Japan Bank', 'BFL', 'Phongsavanh Bank', 'Lao-Viet Bank', 'ACLEDA']" :key="b" :value="b" /></datalist></div>
              <div><label for="b-holder" class="label">{{ $t('kyb.bank.holder') }}</label><UInput id="b-holder" v-model="f.bank_account_name" maxlength="120" /></div>
              <div><label for="b-no" class="label">{{ $t('kyb.bank.number') }}</label><UInput id="b-no" v-model="f.bank_account_number" maxlength="40" class="font-mono" autocomplete="off" :placeholder="f.bank_last4 ? $t('kyb.savedKeep', { last4: f.bank_last4 }) : ''" /></div>
            </div>
            <p class="text-xs text-muted">{{ $t('kyb.bank.nameMatch') }}</p>
          </section>
        </fieldset>

        <!-- 4. documents -->
        <section class="card space-y-4 p-6">
          <div>
            <h2 class="font-bold">4 · {{ $t('kyb.docs.title') }}</h2>
            <p class="text-sm text-muted">{{ $t('kyb.docs.hint', { mb: data.options.max_doc_mb }) }}</p>
          </div>
          <div v-for="k in docKinds" :key="k" class="rounded-2xl border border-line p-4" :data-kind="k">
            <div class="flex flex-wrap items-center gap-2">
              <span class="font-semibold">{{ $t(`kyb.doc.${k}`) }}</span>
              <span class="chip" :class="required.includes(k) ? 'bg-brand-50 text-brand-700' : 'bg-paper text-muted'">{{ required.includes(k) ? $t('kyb.docs.required') : $t('common.optional') }}</span>
              <span v-if="required.includes(k) && docsBy(k).some((d) => d.status !== 'rejected')" class="text-sm text-mint-500">✓</span>
            </div>
            <p class="mt-1 text-xs text-muted">{{ $t(`kyb.docHint.${k}`) }}</p>
            <ul v-if="docsBy(k).length" class="mt-3 space-y-2">
              <li v-for="d in docsBy(k)" :key="d.id" class="flex flex-wrap items-center gap-2 rounded-xl bg-paper px-3 py-2 text-sm">
                <span>{{ d.content_type === 'application/pdf' ? '📄' : '🖼' }}</span>
                <span class="min-w-0 flex-1 truncate">{{ d.original_name || $t(`kyb.doc.${k}`) }} <span class="text-xs text-muted">· {{ kb(d.size_bytes) }}<template v-if="d.expires_on"> · {{ $t('kyb.docs.expires', { date: fmtDate(d.expires_on) }) }}</template></span></span>
                <span class="chip" :class="docStatusCls[d.status]">{{ $t(`kyb.docStatus.${d.status}`) }}</span>
                <button type="button" class="font-semibold text-brand-600 hover:underline" @click="viewing = d">{{ $t('common.view') }}</button>
                <button v-if="!locked && d.status !== 'accepted'" type="button" class="font-semibold text-muted hover:text-ink" @click="removeDoc(d)">{{ $t('common.delete') }}</button>
                <p v-if="d.status === 'rejected' && d.review_note" class="w-full text-xs text-brand-700">{{ d.review_note }}</p>
              </li>
            </ul>
            <div v-if="!locked" class="mt-3 flex flex-wrap items-end gap-2">
              <div>
                <label :for="`exp-${k}`" class="label">{{ $t('kyb.docs.expiry') }}</label>
                <UInput size="md" :id="`exp-${k}`" v-model="u(k).expires" type="date" />
              </div>
              <label class="btn-dark btn-sm cursor-pointer" :class="u(k).busy && 'pointer-events-none opacity-60'">
                {{ u(k).busy ? $t('kyb.docs.uploading') : $t('kyb.docs.upload') }}
                <input type="file" accept="image/jpeg,image/png,image/webp,application/pdf" class="sr-only" :aria-label="$t('kyb.docs.uploadFor', { doc: $t(`kyb.doc.${k}`) })" @change="upload(k, $event)">
              </label>
              <p v-if="u(k).error" class="w-full text-xs text-brand-700" role="alert">{{ u(k).error }}</p>
            </div>
          </div>
        </section>

        <!-- 5. submit -->
        <section v-if="!locked" class="card z-10 flex flex-wrap items-center gap-3 p-4 shadow-lg sm:sticky sm:bottom-3">
          <label class="flex w-full min-w-0 items-start gap-2 text-xs text-ink/80 md:w-auto md:flex-1"><input required type="checkbox" class="mt-0.5 size-4 accent-brand-500"> {{ $t('kyb.declaration') }}</label>
          <UButton color="neutral" variant="soft" type="button" :disabled="busy" @click="save()">{{ $t('kyb.saveDraft') }}</UButton>
          <UButton type="submit" :disabled="busy">{{ busy ? $t('common.saving') : $t('kyb.submit') }}</UButton>
          <p v-if="msg" class="w-full text-sm text-mint-500" role="status">{{ msg }}</p>
          <p v-if="error" class="w-full text-sm text-brand-700" role="alert">{{ error }}</p>
        </section>
        <p v-else-if="msg" class="text-sm text-mint-500" role="status">{{ msg }}</p>
      </form>

      <!-- history -->
      <section v-if="data.events.length" class="card mt-5 p-5">
        <h2 class="font-bold">{{ $t('kyb.history') }}</h2>
        <ol class="mt-3 space-y-2 text-sm">
          <li v-for="(e, i) in data.events" :key="i" class="flex flex-wrap gap-x-2">
            <span class="text-muted">{{ fmtDateTime(e.created_at) }}</span>
            <span class="font-semibold">{{ $te(`kyb.event.${e.action}`) ? $t(`kyb.event.${e.action}`) : e.action }}</span>
            <span v-if="e.action.startsWith('document_') && e.note" class="text-muted">· {{ $te(`kyb.doc.${e.note}`) ? $t(`kyb.doc.${e.note}`) : e.note }}</span>
            <span v-else-if="e.note" class="text-muted">· {{ e.note }}</span>
          </li>
        </ol>
      </section>
      <KybDocViewer v-if="viewing" :doc-id="viewing.id" :title="viewing.original_name || $t(`kyb.doc.${viewing.kind}`)" @close="viewing = null" />
    </template>
  </div>
</template>
