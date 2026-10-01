<script setup lang="ts">
/**
 * Seller: appoint zaokaiy as tax agent (accept the policy), see this month's estimate from every
 * service and the monthly filings made on the shop's behalf.
 */
import { FILING_COLOR, monthLabel, sourceLabel, type ShopFinance } from '#finance/utils/finance'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId } = useShop()
const { locale } = useI18n()
const { data, refresh } = await useAsyncData('shop-finance', () => api<ShopFinance>(`/shops/${shopId.value}/finance`), { watch: [shopId] })

const policy = computed(() => (locale.value === 'lo' && data.value?.settings.policy_lo ? data.value.settings.policy_lo : data.value?.settings.policy_en ?? ''))
const active = computed(() => data.value?.mandate?.status === 'active')
const form = reactive({ signer_name: '', signer_title: '', vat_registered: false, accept: false })
watch(data, (d) => {
  if (d?.mandate) Object.assign(form, { signer_name: d.mandate.signer_name, signer_title: d.mandate.signer_title, vat_registered: d.mandate.vat_registered })
}, { immediate: true })
const error = ref('')
async function accept() {
  error.value = ''
  try {
    await api(`/shops/${shopId.value}/finance/mandate`, { method: 'POST', body: { ...form, policy_version: data.value!.settings.policy_version } })
    form.accept = false
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
const revokeOpen = ref(false)
const reason = ref('')
async function revoke() {
  error.value = ''
  try {
    await api(`/shops/${shopId.value}/finance/mandate/revoke`, { method: 'POST', body: { reason: reason.value } })
    revokeOpen.value = false
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
</script>

<template>
  <div v-if="data" class="max-w-4xl">
    <h1 class="page-title">{{ $t('finance.seller.title') }}</h1>
    <p class="mt-1 text-sm text-muted">{{ $t('finance.seller.subtitle', { agent: data.settings.agent_name }) }}</p>

    <div class="mt-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
      <StatCard :label="$t('finance.rates.vat')" :value="pct(data.settings.vat_bps)" :hint="$t('finance.rates.vatHint')" />
      <StatCard :label="$t('finance.rates.ecommerce')" :value="pct(data.settings.ecommerce_tax_bps)" :hint="$t('finance.rates.ecommerceHint')" />
      <StatCard :label="$t('finance.rates.due')" :value="$t('finance.rates.dueDay', { n: data.settings.due_day })" :hint="$t('finance.rates.dueHint')" />
      <StatCard :label="$t('finance.seller.status')" :value="active ? $t('finance.mandate.active') : $t('finance.mandate.none')" :hint="active ? $t('finance.mandate.since', { date: fmtDate(data.mandate!.accepted_at) }) : $t('finance.mandate.noneHint')" :accent="active" />
    </div>

    <!-- mandate -->
    <section class="card mt-4 p-6">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <h2 class="text-lg font-bold">{{ $t('finance.mandate.title') }}</h2>
        <UBadge :color="active ? 'success' : 'neutral'" :label="active ? $t('finance.mandate.active') : data.mandate ? $t('finance.mandate.revoked') : $t('finance.mandate.none')" />
      </div>
      <p v-if="data.policy_outdated" class="mt-3 rounded-xl bg-sun-400/20 p-3 text-sm font-semibold text-amber-800" role="status">{{ $t('finance.mandate.outdated') }}</p>
      <div class="mt-3 max-h-72 overflow-y-auto whitespace-pre-line rounded-2xl bg-paper p-4 text-sm leading-relaxed" data-testid="policy">{{ policy }}</div>
      <div class="mt-1 text-xs text-muted">{{ $t('finance.mandate.version', { v: data.settings.policy_version }) }}</div>

      <div v-if="!data.shop.tax_id" class="mt-4 rounded-xl bg-brand-50 p-3 text-sm">
        {{ $t('finance.mandate.needTaxId') }} <NuxtLink to="/dashboard/settings" class="font-semibold text-brand-600 underline">{{ $t('common.dashNav.settings') }}</NuxtLink>
      </div>
      <form v-if="!active || data.policy_outdated" class="mt-4 space-y-3" @submit.prevent="accept">
        <div class="grid gap-3 sm:grid-cols-2">
          <div><label class="label" for="md-name">{{ $t('finance.mandate.signer') }}</label><UInput id="md-name" v-model="form.signer_name" required maxlength="120" /></div>
          <div><label class="label" for="md-title">{{ $t('finance.mandate.signerTitle') }}</label><UInput id="md-title" v-model="form.signer_title" maxlength="80" :placeholder="$t('finance.mandate.signerTitlePh')" /></div>
        </div>
        <label class="flex items-start gap-3 text-sm"><input v-model="form.vat_registered" type="checkbox" class="mt-0.5 size-5 accent-brand-500"><span><b>{{ $t('finance.mandate.vatRegistered') }}</b><br><span class="text-muted">{{ $t('finance.mandate.vatRegisteredHint') }}</span></span></label>
        <label class="flex items-start gap-3 text-sm"><input v-model="form.accept" type="checkbox" required class="mt-0.5 size-5 accent-brand-500"><span>{{ $t('finance.mandate.agree', { agent: data.settings.agent_name }) }}</span></label>
        <p v-if="error" class="text-sm text-brand-700" role="alert">{{ error }}</p>
        <UButton type="submit" :disabled="!form.accept">{{ $t('finance.mandate.accept') }}</UButton>
      </form>
      <div v-else class="mt-4 flex flex-wrap items-center gap-3 text-sm">
        <span>{{ $t('finance.mandate.signedBy', { name: data.mandate!.signer_name, date: fmtDate(data.mandate!.accepted_at) }) }} · {{ data.mandate!.vat_registered ? $t('finance.mandate.vatYes') : $t('finance.mandate.vatNo') }} · {{ $t('finance.mandate.taxId', { id: data.mandate!.tax_id }) }}</span>
        <UButton size="sm" color="neutral" variant="ghost" @click="revokeOpen = true">{{ $t('finance.mandate.revoke') }}</UButton>
      </div>
      <UModal v-model:open="revokeOpen" :title="$t('finance.mandate.revoke')">
        <template #body>
          <form class="space-y-3" @submit.prevent="revoke">
            <p class="text-sm text-muted">{{ $t('finance.mandate.revokeHint') }}</p>
            <UInput v-model="reason" :placeholder="$t('finance.mandate.reason')" :aria-label="$t('finance.mandate.reason')" />
            <UButton type="submit" color="error" block>{{ $t('finance.mandate.revokeConfirm') }}</UButton>
          </form>
        </template>
      </UModal>
    </section>

    <!-- this month -->
    <section class="card mt-4 p-6">
      <h2 class="text-lg font-bold">{{ $t('finance.preview.title', { month: monthLabel(data.preview.period) }) }}</h2>
      <p class="text-sm text-muted">{{ data.preview.vat_registered ? $t('finance.preview.vatIncluded') : $t('finance.preview.noVat') }}</p>
      <p v-if="!data.preview.lines.length" class="mt-3 text-sm text-muted">{{ $t('finance.preview.noSales') }}</p>
      <div v-for="l in data.preview.lines" :key="l.currency" class="mt-4 overflow-x-auto">
        <table class="w-full text-sm">
          <tbody>
            <tr v-for="s in l.sources" :key="s.key" class="border-b border-line">
              <td class="py-2">{{ sourceLabel(s) }} <span class="text-xs text-muted">· {{ $t('finance.preview.count', { n: s.count }) }}</span></td>
              <td class="py-2 text-right">{{ money(s.gross_cents, l.currency) }}</td>
            </tr>
            <tr><td class="pt-3 font-semibold">{{ $t('finance.col.gross') }}</td><td class="pt-3 text-right font-semibold">{{ money(l.taxes.gross_cents, l.currency) }}</td></tr>
            <tr><td class="text-muted">{{ $t('finance.col.vat') }}</td><td class="text-right text-muted">{{ money(l.taxes.vat_cents, l.currency) }}</td></tr>
            <tr><td class="text-muted">{{ $t('finance.col.ecommerce') }}</td><td class="text-right text-muted">{{ money(l.taxes.ecommerce_tax_cents, l.currency) }}</td></tr>
            <tr class="text-base"><td class="pt-2 font-extrabold">{{ $t('finance.col.due') }}</td><td class="pt-2 text-right font-extrabold" data-testid="preview-due">{{ money(l.taxes.total_due_cents, l.currency) }}</td></tr>
          </tbody>
        </table>
      </div>
    </section>

    <!-- filings -->
    <section class="card mt-4 p-6">
      <h2 class="text-lg font-bold">{{ $t('finance.filings.title') }}</h2>
      <p v-if="!data.filings.length" class="mt-2 text-sm text-muted">{{ $t('finance.filings.empty') }}</p>
      <div v-else class="mt-3 overflow-x-auto">
        <table class="w-full min-w-[560px] text-sm">
          <thead class="text-left text-xs text-muted"><tr><th class="py-2">{{ $t('finance.col.period') }}</th><th class="text-right">{{ $t('finance.col.gross') }}</th><th class="text-right">{{ $t('finance.col.due') }}</th><th>{{ $t('finance.col.status') }}</th><th>{{ $t('finance.col.reference') }}</th></tr></thead>
          <tbody>
            <tr v-for="f in data.filings" :key="f.id" class="border-t border-line">
              <td class="py-2.5">{{ monthLabel(f.period) }}</td>
              <td class="text-right">{{ money(f.gross_cents, f.currency) }}</td>
              <td class="text-right font-semibold">{{ money(f.total_due_cents, f.currency) }}</td>
              <td class="px-2"><UBadge :color="FILING_COLOR[f.status]" :label="$t(`finance.status.${f.status}`)" /></td>
              <td class="text-xs">{{ f.reference_no || '—' }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  </div>
</template>
