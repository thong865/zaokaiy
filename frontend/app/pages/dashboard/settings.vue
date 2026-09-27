<script setup lang="ts">
import type { Shop } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shop, shops, createShop } = useShop()
const { locale } = useI18n()
const currencies = ['THB', 'LAK', 'USD']
const f = reactive({ name: '', description: '', logo_url: '', kind: 'seller', currency: 'THB' })
const biz = reactive({ legal_name: '', tax_id: '', branch: '', address: '', phone: '', vat: 7, prices_include_vat: true, receipt_prefix: 'R', invoice_prefix: 'INV', receipt_footer: '' })
watch(shop, (s) => {
  if (!s) return
  Object.assign(f, { name: s.name, description: s.description, logo_url: s.logo_url ?? '', kind: s.kind, currency: s.currency || 'THB' })
  Object.assign(biz, { legal_name: s.legal_name, tax_id: s.tax_id, branch: s.branch, address: s.address, phone: s.phone, vat: s.vat_bps / 100,
    prices_include_vat: s.prices_include_vat, receipt_prefix: s.receipt_prefix, invoice_prefix: s.invoice_prefix, receipt_footer: s.receipt_footer })
}, { immediate: true })
const bizMsg = ref('')
async function saveBiz() {
  bizMsg.value = error.value = ''
  try {
    const { vat, ...rest } = biz
    const updated = await api<Shop>(`/shops/${shop.value!.id}`, { method: 'PATCH', body: { ...rest, vat_bps: Math.round(Number(vat) * 100) } })
    const i = shops.value.findIndex((s) => s.id === updated.id)
    shops.value[i] = updated
    bizMsg.value = 'common.saved'
  } catch (e) {
    error.value = apiError(e)
  }
}
const msg = ref('')
const error = ref('')
async function save() {
  msg.value = error.value = ''
  try {
    const updated = await api<Shop>(`/shops/${shop.value!.id}`, { method: 'PATCH', body: { ...f, logo_url: f.logo_url || null } })
    const i = shops.value.findIndex((s) => s.id === updated.id)
    shops.value[i] = updated
    msg.value = 'common.saved'
  } catch (e) {
    error.value = apiError(e)
  }
}
const n = reactive({ name: '', slug: '', kind: 'creator', currency: locale.value === 'lo' ? 'LAK' : 'THB' })
async function addShop() {
  error.value = ''
  try {
    await createShop({ ...n } as never)
    Object.assign(n, { name: '', slug: '' })
  } catch (e) {
    error.value = apiError(e)
  }
}
</script>

<template>
  <div class="max-w-2xl">
    <h1 class="page-title">{{ $t('dash.settings.title') }}</h1>
    <form class="card mt-6 space-y-4 p-6" @submit.prevent="save">
      <div><label class="label">{{ $t('common.name') }}</label><input v-model="f.name" required class="input" ></div>
      <div><label class="label">{{ $t('dash.settings.url') }}</label><input :value="`/s/${shop?.slug}`" disabled class="input bg-paper" ></div>
      <div><label class="label">{{ $t('dash.settings.about') }}</label><textarea v-model="f.description" rows="4" class="input" /></div>
      <div><label class="label">{{ $t('dash.settings.logoUrl') }}</label><input v-model="f.logo_url" class="input" ></div>
      <div>
        <label class="label">{{ $t('dash.settings.shopType') }}</label>
        <select v-model="f.kind" class="input"><option value="seller">{{ $t('dash.settings.typeSeller') }}</option><option value="creator">{{ $t('dash.settings.typeCreator') }}</option></select>
      </div>
      <div>
        <label class="label">{{ $t('dash.settings.currency') }}</label>
        <select v-model="f.currency" class="input">
          <option v-for="c in currencies" :key="c" :value="c">{{ $t(`dash.settings.currencies.${c}`) }}</option>
        </select>
        <p class="mt-1 text-xs text-muted">{{ $t('dash.settings.currencyHint') }}</p>
      </div>
      <p v-if="msg" class="text-sm text-mint-500">{{ $t(msg) }}</p>
      <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
      <button class="btn-primary">{{ $t('common.save') }}</button>
    </form>

    <form class="card mt-6 space-y-4 p-6" @submit.prevent="saveBiz">
      <div>
        <div class="font-bold">{{ $t('dash.settings.biz.title') }}</div>
        <p class="text-sm text-muted">{{ $t('dash.settings.biz.subtitle') }}</p>
      </div>
      <div class="grid gap-3 sm:grid-cols-2">
        <div class="sm:col-span-2"><label class="label">{{ $t('dash.settings.biz.legalName') }}</label><input v-model="biz.legal_name" class="input" :placeholder="$t('dash.settings.biz.legalNamePh')" ></div>
        <div><label class="label">{{ $t('dash.settings.biz.taxId') }}</label><input v-model="biz.tax_id" class="input" :placeholder="$t('dash.settings.biz.taxIdPh')" inputmode="numeric" ></div>
        <div><label class="label">{{ $t('dash.settings.biz.branch') }}</label><input v-model="biz.branch" class="input" :placeholder="$t('dash.settings.biz.branchPh')" ></div>
        <div class="sm:col-span-2"><label class="label">{{ $t('common.address') }}</label><textarea v-model="biz.address" rows="2" class="input" /></div>
        <div><label class="label">{{ $t('common.phone') }}</label><input v-model="biz.phone" class="input" ></div>
        <div>
          <label class="label">{{ $t('dash.settings.biz.vatRate') }}</label>
          <input v-model="biz.vat" type="number" min="0" max="30" step="0.01" class="input" >
        </div>
        <label class="flex items-center gap-2 text-sm sm:col-span-2"><input v-model="biz.prices_include_vat" type="checkbox" class="size-4 accent-brand-500"> {{ $t('dash.settings.biz.pricesIncludeVat') }}</label>
        <div><label class="label">{{ $t('dash.settings.biz.receiptPrefix') }}</label><input v-model="biz.receipt_prefix" maxlength="8" class="input font-mono" ></div>
        <div><label class="label">{{ $t('dash.settings.biz.invoicePrefix') }}</label><input v-model="biz.invoice_prefix" maxlength="8" class="input font-mono" ></div>
        <div class="sm:col-span-2"><label class="label">{{ $t('dash.settings.biz.receiptFooter') }}</label><textarea v-model="biz.receipt_footer" rows="2" class="input" :placeholder="$t('dash.settings.biz.receiptFooterPh')" /></div>
      </div>
      <p v-if="bizMsg" class="text-sm text-mint-500">{{ $t(bizMsg) }}</p>
      <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
      <button class="btn-primary">{{ $t('dash.settings.biz.save') }}</button>
    </form>

    <form class="card mt-6 space-y-3 p-6" @submit.prevent="addShop">
      <div class="font-bold">{{ $t('dash.settings.another.title') }}</div>
      <p class="text-sm text-muted">{{ $t('dash.settings.another.subtitle') }}</p>
      <div class="grid gap-3 sm:grid-cols-2">
        <input v-model="n.name" required :placeholder="$t('common.name')" class="input" >
        <input v-model="n.slug" required pattern="[a-z0-9-]{3,40}" :placeholder="$t('dash.settings.another.slugPh')" class="input" >
        <select v-model="n.kind" class="input"><option value="seller">{{ $t('dash.open.seller') }}</option><option value="creator">{{ $t('dash.open.creator') }}</option></select>
        <select v-model="n.currency" class="input">
          <option v-for="c in currencies" :key="c" :value="c">{{ $t(`dash.settings.currencies.${c}`) }}</option>
        </select>
      </div>
      <button class="btn-ghost">{{ $t('dash.open.create') }}</button>
    </form>
  </div>
</template>
