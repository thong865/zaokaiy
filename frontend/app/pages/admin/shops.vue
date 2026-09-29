<script setup lang="ts">
definePageMeta({ layout: 'admin', middleware: 'admin' })
const api = useApi()
interface ShopRow { id: string; name: string; slug: string; kind: string; auto_approve: boolean; created_at: string; owner_email: string; products: number; pending: number; rejected: number }
const q = ref('')
const { data: shops, refresh } = await useAsyncData('admin-shops', () => api<ShopRow[]>('/admin/shops', { query: { q: q.value || undefined } }), { default: () => [] })
let t: ReturnType<typeof setTimeout> | undefined
watch(q, () => { clearTimeout(t); t = setTimeout(refresh, 250) })
const error = ref('')
async function setTrusted(s: ShopRow, v: boolean) {
  error.value = ''
  try {
    await api(`/admin/shops/${s.id}`, { method: 'PATCH', body: { auto_approve: v } })
    s.auto_approve = v
  } catch (e) {
    error.value = apiError(e)
  }
}
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('admin.shops.title') }}</h1>
    <i18n-t keypath="admin.shops.intro" tag="p" class="text-sm text-muted"><template #trusted><b>{{ $t('admin.shops.trustedWord') }}</b></template></i18n-t>
    <UInput v-model="q" class="mt-5 w-72" :placeholder="$t('admin.shops.searchPlaceholder')" />
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>
    <div class="card mt-4 overflow-x-auto">
      <table class="table">
        <thead><tr><th>{{ $t('admin.shops.cols.shop') }}</th><th>{{ $t('admin.shops.cols.owner') }}</th><th>{{ $t('admin.shops.cols.products') }}</th><th>{{ $t('admin.shops.cols.pending') }}</th><th>{{ $t('admin.shops.cols.rejected') }}</th><th>{{ $t('admin.shops.cols.trusted') }}</th></tr></thead>
        <tbody>
          <tr v-for="s in shops" :key="s.id">
            <td>
              <NuxtLink :to="`/s/${s.slug}`" target="_blank" class="font-semibold hover:underline">{{ s.name }}</NuxtLink>
              <div class="text-xs capitalize text-muted">{{ $te(`store.shopKind.${s.kind}`) ? $t(`store.shopKind.${s.kind}`) : s.kind }} · {{ $t('admin.shops.since', { date: fmtDate(s.created_at) }) }}</div>
            </td>
            <td class="text-muted">{{ s.owner_email }}</td>
            <td>{{ s.products }}</td>
            <td><NuxtLink v-if="s.pending" :to="{ path: '/admin', query: { tab: 'pending' } }" class="font-bold text-amber-700 hover:underline">{{ s.pending }}</NuxtLink><span v-else class="text-muted">0</span></td>
            <td :class="s.rejected ? 'text-brand-700' : 'text-muted'">{{ s.rejected }}</td>
            <td>
              <label class="relative inline-flex cursor-pointer items-center">
                <input type="checkbox" class="peer sr-only" :checked="s.auto_approve" :aria-label="$t('admin.shops.trust', { name: s.name })" @change="setTrusted(s, ($event.target as HTMLInputElement).checked)">
                <span class="h-6 w-11 rounded-full bg-line transition peer-checked:bg-mint-500" />
                <span class="absolute left-0.5 top-0.5 size-5 rounded-full bg-surface shadow transition peer-checked:translate-x-5" />
              </label>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
