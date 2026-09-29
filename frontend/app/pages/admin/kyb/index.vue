<script setup lang="ts">
import type { KybStatus } from '~/utils/types'

definePageMeta({ layout: 'admin', middleware: 'admin' })
const api = useApi()
interface Row { shop_id: string; shop_name: string; slug: string; kyb_status: KybStatus; kyb_verified_at: string | null; owner: string; legal_name: string | null; company_type: string | null; registration_no: string | null; country: string | null; submitted_at: string | null; reviewed_at: string | null; review_due_at: string | null; risk_flags: string[]; documents: number }
const tabs: ('submitted' | 'changes_requested' | 'approved' | 'rejected' | 'revoked' | 'draft' | 'all')[] = ['submitted', 'changes_requested', 'approved', 'rejected', 'revoked', 'draft', 'all']
const status = ref<(typeof tabs)[number]>('submitted')
const q = ref('')
const search = ref('')
const { data } = await useAsyncData('admin-kyb', () => api<{ items: Row[]; counts: Record<string, number> }>('/admin/kyb', { query: { status: status.value, q: search.value || undefined } }), {
  watch: [status, search],
  default: () => ({ items: [] as Row[], counts: {} as Record<string, number> }),
})
const total = computed(() => Object.values(data.value.counts).reduce((a, n) => a + n, 0))
const now = Date.now()
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('kyb.admin.title') }}</h1>
    <p class="text-sm text-muted">{{ $t('kyb.admin.subtitle') }}</p>
    <div class="mt-5 flex flex-wrap items-center gap-2">
      <button v-for="tb in tabs" :key="tb" class="chip border px-3 py-1.5" :class="status === tb ? 'border-brand-500 bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]' : 'border-transparent bg-surface shadow-card hover:text-brand-500'" @click="status = tb">
        {{ tb === 'all' ? $t('common.all') : $t(`kyb.status.${tb}`) }} <span class="opacity-60">{{ tb === 'all' ? total : data.counts[tb] ?? 0 }}</span>
      </button>
      <form class="ml-auto" @submit.prevent="search = q.trim()"><UInput size="md" v-model="q" class="w-64" :placeholder="$t('kyb.admin.search')" :aria-label="$t('kyb.admin.search')" /></form>
    </div>
    <div v-if="data.items.length" class="card mt-5 overflow-x-auto">
      <table class="w-full text-sm">
        <thead class="border-b border-line text-left text-xs uppercase tracking-wider text-muted">
          <tr><th class="p-3">{{ $t('kyb.admin.company') }}</th><th class="p-3">{{ $t('kyb.company.registrationNo') }}</th><th class="p-3">{{ $t('common.shop') }}</th><th class="p-3">{{ $t('common.statusLabel') }}</th><th class="p-3">{{ $t('kyb.admin.flags') }}</th><th class="p-3">{{ $t('kyb.admin.submitted') }}</th></tr>
        </thead>
        <tbody>
          <tr v-for="r in data.items" :key="r.shop_id" class="border-b border-line/60 last:border-0 hover:bg-paper/60">
            <td class="p-3"><NuxtLink :to="`/admin/kyb/${r.shop_id}`" class="font-semibold hover:underline">{{ r.legal_name || '—' }}</NuxtLink><div class="text-xs text-muted">{{ r.company_type ? $t(`kyb.companyType.${r.company_type}`) : '' }} · {{ r.country }}</div></td>
            <td class="p-3 font-mono text-xs">{{ r.registration_no || '—' }}</td>
            <td class="p-3">{{ r.shop_name }}<div class="text-xs text-muted">{{ r.owner }}</div></td>
            <td class="p-3"><KybStatusBadge :status="r.kyb_status" :verified="!!r.kyb_verified_at" /><div v-if="r.review_due_at && new Date(r.review_due_at).getTime() < now" class="mt-1 text-xs font-semibold text-brand-700">{{ $t('kyb.admin.reviewOverdue') }}</div></td>
            <td class="p-3"><span v-for="fl in r.risk_flags" :key="fl" class="chip mb-1 mr-1 bg-sun-400/30 text-amber-800">{{ $t(`kyb.flag.${fl}`) }}</span><span v-if="!r.risk_flags.length" class="text-muted">—</span></td>
            <td class="p-3 text-xs text-muted">{{ r.submitted_at ? ago(r.submitted_at) : '—' }}<div>{{ $t('kyb.admin.docs', { n: r.documents }) }}</div></td>
          </tr>
        </tbody>
      </table>
    </div>
    <EmptyState v-else class="mt-6" :title="$t('kyb.admin.empty')" />
  </div>
</template>
