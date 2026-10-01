<script setup lang="ts">
import type { AdCampaign } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId, shop } = useShop()
const { t, locale } = useI18n()
const { data: ads, refresh } = await useAsyncData('ads-list', () => api<AdCampaign[]>(`/shops/${shopId.value}/ads`), { watch: [shopId], default: () => [] })
const { data: sellable } = await useSellable()
const cur = computed(() => shop.value?.currency ?? 'THB')
const name = (id: string) => sellable.value.find((s) => s.id === id)?.name ?? t('common.product')

const show = ref(false)
const f = reactive({ product_id: '', headline: '', body: '', image_url: '', budget: 500, cpc: 2, status: 'active' })
const error = ref('')
const aiBusy = ref(false)
async function aiCopy() {
  if (!f.product_id) return (error.value = t('grow.ads.chooseProductFirst'))
  aiBusy.value = true
  error.value = ''
  try {
    const r = await api<{ title: string; body: string }>(`/shops/${shopId.value}/ai/generate`, { method: 'POST', body: { product_id: f.product_id, kind: 'ad_copy', language: locale.value === 'lo' ? 'Lao' : 'English' } })
    f.headline = r.title.slice(0, 80)
    f.body = r.body.replace(/\n\n\[offline template.*$/s, '').slice(0, 200)
  } catch (e) {
    error.value = apiError(e)
  } finally {
    aiBusy.value = false
  }
}
async function create() {
  error.value = ''
  try {
    await api(`/shops/${shopId.value}/ads`, {
      method: 'POST',
      body: { product_id: f.product_id, headline: f.headline, body: f.body, image_url: f.image_url || null, budget_cents: Math.round(f.budget * 100), cpc_cents: Math.round(f.cpc * 100), status: f.status },
    })
    show.value = false
    Object.assign(f, { product_id: '', headline: '', body: '', image_url: '' })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
async function setStatus(a: AdCampaign, status: string) {
  try {
    await api(`/ads/${a.id}`, { method: 'PATCH', body: { status } })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
</script>

<template>
  <div>
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h1 class="page-title">{{ $t('grow.ads.title') }}</h1>
        <p class="text-sm text-muted">{{ $t('grow.ads.intro') }}</p>
      </div>
      <UButton type="submit" @click="show = true">+ {{ $t('grow.ads.newCampaign') }}</UButton>
    </div>
    <p v-if="error && !show" class="mt-3 text-sm text-brand-700">{{ error }}</p>

    <div v-if="ads.length" class="card mt-6 overflow-x-auto">
      <table class="table">
        <thead><tr><th>{{ $t('grow.ads.campaign') }}</th><th>{{ $t('grow.ads.budget') }}</th><th>{{ $t('grow.ads.impressions') }}</th><th>{{ $t('grow.ads.clicks') }}</th><th>CTR</th><th>{{ $t('grow.status') }}</th><th /></tr></thead>
        <tbody>
          <tr v-for="a in ads" :key="a.id">
            <td><div class="font-semibold">{{ a.headline }}</div><div class="text-xs text-muted">{{ name(a.product_id) }} · {{ $t('grow.ads.perClickPrice', { price: money(a.cpc_cents, cur) }) }}</div></td>
            <td>
              <div class="text-xs">{{ money(a.spent_cents, cur) }} / {{ money(a.budget_cents, cur) }}</div>
              <div class="mt-1 h-1.5 w-28 rounded-full bg-line"><div class="h-full rounded-full bg-brand-500" :style="{ width: `${Math.min(100, (a.spent_cents / a.budget_cents) * 100)}%` }" /></div>
            </td>
            <td>{{ num(a.impressions) }}</td>
            <td>{{ num(a.clicks) }}</td>
            <td>{{ a.impressions ? ((a.clicks / a.impressions) * 100).toFixed(1) + '%' : '—' }}</td>
            <td><StatusBadge :status="a.status" /></td>
            <td class="whitespace-nowrap text-right">
              <UButton color="neutral" variant="soft" size="sm" type="submit" v-if="a.status === 'active'" @click="setStatus(a, 'paused')">{{ $t('grow.ads.pause') }}</UButton>
              <UButton size="sm" type="submit" v-else-if="a.status !== 'ended'" @click="setStatus(a, 'active')">{{ $t('grow.ads.activate') }}</UButton>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <EmptyState v-else class="mt-6" :title="$t('grow.ads.emptyTitle')" :text="$t('grow.ads.emptyText')" />

    <div v-if="show" class="fixed inset-0 z-50 grid place-items-center bg-night/40 p-4" @click.self="show = false">
      <form class="card w-full max-w-lg space-y-4 p-6 shadow-2xl" @submit.prevent="create">
        <div class="text-lg font-bold">{{ $t('grow.ads.newCampaign') }}</div>
        <div>
          <label class="label">{{ $t('common.product') }}</label>
          <select v-model="f.product_id" required class="input">
            <option value="" disabled>{{ $t('grow.choose') }}</option>
            <option v-for="s in sellable" :key="s.id" :value="s.id">{{ s.name }}{{ s.resold ? ` (${$t('grow.resold')})` : '' }}</option>
          </select>
        </div>
        <div>
          <div class="flex items-center justify-between"><label class="label">{{ $t('grow.ads.headline') }}</label><button type="button" class="mb-1.5 text-xs font-semibold text-brand-600" :disabled="aiBusy" @click="aiCopy">{{ aiBusy ? $t('grow.ads.writing') : '✦ ' + $t('grow.ads.writeWithAi') }}</button></div>
          <UInput v-model="f.headline" required maxlength="80" />
        </div>
        <div><label class="label">{{ $t('grow.ads.body') }}</label><UTextarea v-model="f.body" :rows="2" maxlength="200" /></div>
        <MediaField v-model="f.image_url" :shop-id="shopId" kind="image" :label="$t('grow.ads.image')" />
        <div class="grid grid-cols-3 gap-3">
          <div><label class="label">{{ $t('grow.ads.budget') }}</label><UInput v-model.number="f.budget" type="number" min="1" step="1" /></div>
          <div><label class="label">{{ $t('grow.ads.perClick') }}</label><UInput v-model.number="f.cpc" type="number" min="0.01" step="0.01" /></div>
          <div><label class="label">{{ $t('grow.ads.startAs') }}</label><select v-model="f.status" class="input"><option value="active">{{ $t('common.status.active') }}</option><option value="draft">{{ $t('common.status.draft') }}</option></select></div>
        </div>
        <p class="text-xs text-muted">≈ {{ $t('grow.ads.estimate', { n: num(Math.floor(f.budget / (f.cpc || 1))), price: money(f.budget * 100, cur) }) }}</p>
        <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
        <div class="flex justify-end gap-2"><UButton color="neutral" variant="soft" type="button" @click="show = false">{{ $t('common.cancel') }}</UButton><UButton type="submit">{{ $t('common.create') }}</UButton></div>
      </form>
    </div>
  </div>
</template>
