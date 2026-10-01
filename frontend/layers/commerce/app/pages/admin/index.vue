<script setup lang="ts">
definePageMeta({ layout: 'admin', middleware: 'admin' })
const api = useApi()
const route = useRoute()
const router = useRouter()

interface Row {
  id: string
  name: string
  sku: string
  price_cents: number
  cover: string | null
  status: string
  review_status: string
  review_note: string
  submitted_at: string | null
  updated_at: string
  shop: { id: string; name: string }
  currency: string
  category: string | null
  stock: number
}
interface Overview { pending: number; rejected: number; approved: number; live: number; shops: number; users: number; categories: number; oldest_pending_at: string | null; review_required: boolean }

const tab = ref((route.query.tab as string) || 'pending')
watch(tab, (t) => router.replace({ query: { ...route.query, tab: t } }))
const q = ref('')
const qd = ref('')
let timer: ReturnType<typeof setTimeout> | undefined
watch(q, (v) => { clearTimeout(timer); timer = setTimeout(() => (qd.value = v), 250) })

const { data: overview, refresh: refreshOverview } = await useAsyncData('admin-overview', () => api<Overview>('/admin/overview'))
const { data: rows, refresh, status } = await useAsyncData(
  'admin-queue',
  () => api<Row[]>('/admin/products', { query: { review_status: tab.value === 'all' ? undefined : tab.value, q: qd.value || undefined, limit: 100 } }),
  { watch: [tab, qd], default: () => [] },
)

const selected = ref<Set<string>>(new Set())
watch(tab, () => (selected.value = new Set()))
const allSelected = computed(() => rows.value.length > 0 && rows.value.every((r) => selected.value.has(r.id)))
function toggleAll() {
  selected.value = allSelected.value ? new Set() : new Set(rows.value.map((r) => r.id))
}
function toggle(id: string) {
  const s = new Set(selected.value)
  s.has(id) ? s.delete(id) : s.add(id)
  selected.value = s
}

const rejecting = ref(false)
const note = ref('')
const error = ref('')
const notice = ref<{ key: string; n: number } | null>(null)
async function bulk(action: 'approve' | 'reject') {
  error.value = ''
  try {
    const r = await api<{ updated: number; failed: unknown[] }>('/admin/products/review', {
      method: 'POST',
      body: { ids: [...selected.value], action, note: action === 'reject' ? note.value : undefined },
    })
    notice.value = { key: action === 'approve' ? 'admin.review.noticeApproved' : 'admin.review.noticeRejected', n: r.updated }
    setTimeout(() => (notice.value = null), 2500)
    selected.value = new Set()
    rejecting.value = false
    note.value = ''
    await Promise.all([refresh(), refreshOverview()])
  } catch (e) {
    error.value = apiError(e)
  }
}
const { t } = useI18n()
const tabs = computed(() => (['pending', 'rejected', 'approved', 'all'] as const).map((k) => [k, t(`admin.review.tabs.${k}`)] as const))
</script>

<template>
  <div>
    <div class="flex flex-wrap items-end justify-between gap-4">
      <div>
        <h1 class="page-title">{{ $t('admin.review.title') }}</h1>
        <p class="text-sm text-muted">
          {{ $t('admin.review.intro') }}
          <span v-if="overview && !overview.review_required" class="font-semibold text-amber-700">{{ $t('admin.review.moderationOff') }}</span>
        </p>
      </div>
      <div v-if="overview" class="flex gap-6 text-sm">
        <div><div class="text-2xl font-extrabold text-amber-700">{{ overview.pending }}</div><div class="text-muted">{{ $t('admin.review.stats.pending') }}</div></div>
        <div><div class="text-2xl font-extrabold">{{ overview.live }}</div><div class="text-muted">{{ $t('admin.review.stats.live') }}</div></div>
        <div><div class="text-2xl font-extrabold">{{ overview.shops }}</div><div class="text-muted">{{ $t('admin.review.stats.shops') }}</div></div>
        <div v-if="overview.oldest_pending_at"><div class="text-2xl font-extrabold">{{ ago(overview.oldest_pending_at) }}</div><div class="text-muted">{{ $t('admin.review.stats.oldestWait') }}</div></div>
      </div>
    </div>

    <div class="mt-6 flex flex-wrap items-center gap-3">
      <div class="flex rounded-2xl bg-surface p-1 shadow-card text-sm font-semibold">
        <button v-for="t in tabs" :key="t[0]" class="rounded-xl px-3.5 py-1" :class="tab === t[0] && 'bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]'" @click="tab = t[0]">{{ t[1] }}</button>
      </div>
      <UInput v-model="q" class="w-64" :placeholder="$t('admin.review.searchPlaceholder')" />
      <div v-if="selected.size" class="ml-auto flex items-center gap-2 text-sm">
        <span class="font-semibold">{{ $t('admin.review.selected', { n: selected.size }) }}</span>
        <UButton size="sm" type="submit" @click="bulk('approve')">{{ $t('admin.review.approve') }}</UButton>
        <UButton color="neutral" variant="soft" size="sm" type="submit" class="text-brand-700" @click="rejecting = true">{{ $t('admin.review.rejectEllipsis') }}</UButton>
      </div>
    </div>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>

    <div v-if="rows.length" class="card mt-4 overflow-x-auto">
      <table class="table">
        <thead>
          <tr>
            <th class="w-10"><input type="checkbox" class="size-4 accent-brand-500" :checked="allSelected" :aria-label="$t('admin.review.selectAll')" @change="toggleAll"></th>
            <th>{{ $t('admin.review.cols.product') }}</th><th>{{ $t('admin.review.cols.shop') }}</th><th>{{ $t('admin.review.cols.category') }}</th><th>{{ $t('admin.review.cols.price') }}</th><th>{{ $t('admin.review.cols.submitted') }}</th><th>{{ $t('admin.review.cols.review') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in rows" :key="r.id" class="hover:bg-paper">
            <td><input type="checkbox" class="size-4 accent-brand-500" :checked="selected.has(r.id)" :aria-label="$t('admin.review.selectRow', { name: r.name })" @change="toggle(r.id)"></td>
            <td>
              <NuxtLink :to="`/admin/products/${r.id}`" class="flex items-center gap-3">
                <ProductThumb :src="r.cover" :name="r.name" class="size-11 shrink-0" rounded="rounded-lg" />
                <div class="min-w-0"><div class="truncate font-semibold hover:underline">{{ r.name }}</div><div class="text-xs text-muted">{{ r.sku }} · {{ $t('admin.review.inStock', { n: r.stock }) }}</div></div>
              </NuxtLink>
            </td>
            <td class="whitespace-nowrap">{{ r.shop.name }}</td>
            <td class="text-muted">{{ r.category ?? '—' }}</td>
            <td class="whitespace-nowrap font-semibold">{{ money(r.price_cents, r.currency) }}</td>
            <td class="whitespace-nowrap text-muted">{{ r.submitted_at ? ago(r.submitted_at) : '—' }}</td>
            <td>
              <StatusBadge :status="r.review_status" :label="reviewLabel(r.review_status)" />
              <div v-if="r.review_note" class="mt-1 max-w-52 truncate text-[11px] text-muted" :title="r.review_note">{{ r.review_note }}</div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <EmptyState v-else-if="status !== 'pending'" class="mt-4" :title="tab === 'pending' ? $t('admin.review.queueClear') : $t('admin.review.nothingHere')" :text="tab === 'pending' ? $t('admin.review.queueClearText') : undefined" />

    <Teleport to="body">
      <div v-if="rejecting" class="fixed inset-0 z-50 grid place-items-center bg-night/40 p-4" @click.self="rejecting = false">
        <form class="card w-full max-w-md space-y-4 p-6 shadow-2xl" @submit.prevent="bulk('reject')">
          <div class="text-lg font-bold">{{ $t('admin.review.rejectTitle', selected.size) }}</div>
          <UTextarea v-model="note" required :rows="4" :placeholder="$t('admin.review.rejectPlaceholder')" />
          <div class="flex justify-end gap-2">
            <UButton color="neutral" variant="soft" type="button" @click="rejecting = false">{{ $t('common.cancel') }}</UButton>
            <UButton color="neutral" variant="ghost" type="submit" class="bg-brand-700 text-white hover:bg-brand-600">{{ $t('admin.review.reject') }}</UButton>
          </div>
        </form>
      </div>
    </Teleport>
    <div v-if="notice" class="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-full bg-ink px-4 py-2 text-sm font-semibold text-paper shadow-lg">{{ $t(notice.key, notice.n) }}</div>
  </div>
</template>
