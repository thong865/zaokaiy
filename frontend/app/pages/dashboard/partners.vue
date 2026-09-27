<script setup lang="ts">
import type { Partnership } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId } = useShop()
const { data: incoming, refresh } = await useAsyncData('partners-in', () => api<Partnership[]>(`/shops/${shopId.value}/partnerships`, { query: { as: 'supplier' } }), { watch: [shopId], default: () => [] })
const { data: mine, refresh: refreshMine } = await useAsyncData('partners-out', () => api<Partnership[]>(`/shops/${shopId.value}/partnerships`, { query: { as: 'reseller' } }), { watch: [shopId], default: () => [] })

const rates = reactive<Record<string, number | undefined>>({})
const error = ref('')
async function decide(p: Partnership, approve: boolean) {
  error.value = ''
  const r = rates[p.id]
  try {
    await api(`/partnerships/${p.id}/decision`, {
      method: 'POST',
      body: { approve, commission_bps: r !== undefined && r !== null && String(r) !== '' ? Math.round(Number(r) * 100) : undefined },
    })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
async function revoke(p: Partnership) {
  try {
    await api(`/partnerships/${p.id}/revoke`, { method: 'POST' })
    await Promise.all([refresh(), refreshMine()])
  } catch (e) {
    error.value = apiError(e)
  }
}
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('network.partners.title') }}</h1>
    <p class="text-sm text-muted">{{ $t('network.partners.intro') }}</p>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>

    <div v-if="incoming.length" class="card mt-6 overflow-x-auto">
      <table class="table">
        <thead><tr><th>{{ $t('network.sellStaff') }}</th><th>{{ $t('network.partners.message') }}</th><th>{{ $t('network.commission') }}</th><th>{{ $t('network.status') }}</th><th /></tr></thead>
        <tbody>
          <tr v-for="p in incoming" :key="p.id">
            <td class="font-semibold">{{ p.reseller_name }}<div class="text-xs font-normal text-muted">{{ ago(p.created_at) }}</div></td>
            <td class="max-w-xs text-muted">{{ p.message || '—' }}</td>
            <td>
              <div v-if="p.status === 'pending' || p.status === 'approved'" class="flex items-center gap-1">
                <input v-model="rates[p.id]" type="number" min="0" max="90" step="0.5" :placeholder="p.commission_bps !== null ? String(p.commission_bps / 100) : $t('network.partners.default')" class="input w-24 px-2 py-1.5" >
                <span class="text-muted">%</span>
              </div>
              <span v-else class="text-muted">{{ p.commission_bps !== null ? pct(p.commission_bps) : $t('network.partners.default') }}</span>
            </td>
            <td><StatusBadge :status="p.status" /></td>
            <td class="whitespace-nowrap text-right">
              <template v-if="p.status === 'pending'">
                <button class="btn-primary btn-sm" @click="decide(p, true)">{{ $t('network.partners.approve') }}</button>
                <button class="btn-ghost btn-sm ml-2" @click="decide(p, false)">{{ $t('network.partners.decline') }}</button>
              </template>
              <template v-else-if="p.status === 'approved'">
                <button class="btn-ghost btn-sm" @click="decide(p, true)">{{ $t('network.partners.updateRate') }}</button>
                <button class="btn-ghost btn-sm ml-2" @click="revoke(p)">{{ $t('network.partners.revoke') }}</button>
              </template>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <EmptyState v-else class="mt-6" :title="$t('network.partners.emptyTitle')" :text="$t('network.partners.emptyText')" >
      <NuxtLink to="/dashboard/products" class="btn-ghost">{{ $t('network.partners.manageProducts') }}</NuxtLink>
    </EmptyState>

    <h2 class="mt-12 font-bold">{{ $t('network.partners.sellFor') }}</h2>
    <div v-if="mine.length" class="card mt-3 overflow-x-auto">
      <table class="table">
        <thead><tr><th>{{ $t('network.supplier') }}</th><th>{{ $t('network.partners.yourRate') }}</th><th>{{ $t('network.status') }}</th><th /></tr></thead>
        <tbody>
          <tr v-for="p in mine" :key="p.id">
            <td class="font-semibold">{{ p.supplier_name }}</td>
            <td>{{ p.commission_bps !== null ? pct(p.commission_bps) : $t('network.partners.productDefault') }}</td>
            <td><StatusBadge :status="p.status" /></td>
            <td class="text-right"><button v-if="p.status === 'approved' || p.status === 'pending'" class="btn-ghost btn-sm" @click="revoke(p)">{{ $t('network.partners.leave') }}</button></td>
          </tr>
        </tbody>
      </table>
    </div>
    <i18n-t v-else keypath="network.partners.noneMine" tag="p" class="mt-3 text-sm text-muted"><template #link><NuxtLink to="/dashboard/marketplace" class="font-semibold text-ink underline">{{ $t('network.partners.browse') }}</NuxtLink></template></i18n-t>
  </div>
</template>
