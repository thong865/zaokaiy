<script setup lang="ts">
import type { MediaAsset } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId } = useShop()
const { t } = useI18n()

const filter = ref<'' | 'image' | 'video' | 'unused'>('')
const q = ref('')
const qd = ref('')
let timer: ReturnType<typeof setTimeout> | undefined
watch(q, (v) => { clearTimeout(timer); timer = setTimeout(() => (qd.value = v), 250) })

interface Library { items: MediaAsset[]; stats: { count: number; bytes: number; images: number; videos: number } }
const { data, refresh, status } = await useAsyncData(
  'media-library',
  () =>
    api<Library>(`/shops/${shopId.value}/media`, {
      query: {
        kind: filter.value === 'image' || filter.value === 'video' ? filter.value : undefined,
        unused: filter.value === 'unused' || undefined,
        q: qd.value || undefined,
        limit: 200,
      },
    }),
  { watch: [shopId, filter, qd] },
)
const items = computed(() => data.value?.items ?? [])

// Selection
const selected = ref<Set<string>>(new Set())
const selectMode = computed(() => selected.value.size > 0)
function toggleSel(a: MediaAsset) {
  const s = new Set(selected.value)
  s.has(a.id) ? s.delete(a.id) : s.add(a.id)
  selected.value = s
}
const allSelected = computed(() => items.value.length > 0 && items.value.every((a) => selected.value.has(a.id)))
function toggleAll() {
  selected.value = allSelected.value ? new Set() : new Set(items.value.map((a) => a.id))
}
watch([shopId, filter], () => (selected.value = new Set()))

const error = ref('')
const notice = ref('')
function flash(msg: string) {
  notice.value = msg
  setTimeout(() => (notice.value = ''), 2500)
}

async function bulkDelete() {
  error.value = ''
  const ids = [...selected.value]
  try {
    await api(`/shops/${shopId.value}/media/delete`, { method: 'POST', body: { ids } })
  } catch (e: unknown) {
    const status = (e as { status?: number }).status ?? (e as { response?: { status: number } }).response?.status
    if (status !== 409) return (error.value = apiError(e))
    if (!window.confirm(`${apiError(e)}.\n\n${t('media.library.forceConfirm')}`)) return
    try {
      await api(`/shops/${shopId.value}/media/delete`, { method: 'POST', body: { ids, force: true } })
    } catch (e2) {
      return (error.value = apiError(e2))
    }
  }
  selected.value = new Set()
  if (active.value && ids.includes(active.value.id)) active.value = null
  flash(t('media.library.deleted', { n: ids.length }, ids.length))
  await refresh()
}

// Detail drawer
interface Detail { asset: MediaAsset; products: { id: string; name: string; is_cover: boolean }[] }
const active = ref<MediaAsset | null>(null)
const detail = ref<Detail | null>(null)
const alt = ref('')
async function openDetail(a: MediaAsset) {
  if (selectMode.value) return toggleSel(a)
  active.value = a
  detail.value = null
  alt.value = a.alt
  detail.value = await api<Detail>(`/media/${a.id}`)
}
async function saveAlt() {
  if (!active.value) return
  await api(`/media/${active.value.id}`, { method: 'PATCH', body: { alt: alt.value } })
  active.value.alt = alt.value
  flash(t('media.library.altSaved'))
}
async function copy(text: string) {
  await navigator.clipboard?.writeText(text)
  flash(t('media.library.urlCopied'))
}
async function deleteActive() {
  if (!active.value) return
  selected.value = new Set([active.value.id])
  await bulkDelete()
}
function onUploaded(assets: MediaAsset[]) {
  flash(t('media.library.uploaded', { n: assets.length }, assets.length))
  refresh()
}
const tabs = computed(() => [
  ['', t('common.all')],
  ['image', t('media.images')],
  ['video', t('media.videos')],
  ['unused', t('media.library.unused')],
] as const)
</script>

<template>
  <div>
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 class="page-title">{{ $t('media.title') }}</h1>
        <p class="text-sm text-muted">{{ $t('media.library.intro') }}</p>
      </div>
      <div v-if="data" class="flex gap-5 text-sm">
        <div><div class="text-lg font-extrabold">{{ data.stats.images }}</div><div class="text-muted">{{ $t('media.images') }}</div></div>
        <div><div class="text-lg font-extrabold">{{ data.stats.videos }}</div><div class="text-muted">{{ $t('media.videos') }}</div></div>
        <div><div class="text-lg font-extrabold">{{ bytes(data.stats.bytes) }}</div><div class="text-muted">{{ $t('media.library.storage') }}</div></div>
      </div>
    </div>

    <MediaUploader v-if="shopId" class="mt-6" :shop-id="shopId" @uploaded="onUploaded" />

    <div class="sticky top-0 z-10 -mx-2 mt-6 flex flex-wrap items-center gap-3 bg-paper/90 px-2 py-2 backdrop-blur">
      <div class="flex rounded-full border border-line bg-white p-1 text-sm font-semibold">
        <button v-for="t in tabs" :key="t[0]" class="rounded-full px-3.5 py-1" :class="filter === t[0] && 'bg-ink text-white'" @click="filter = t[0]">{{ t[1] }}</button>
      </div>
      <input v-model="q" class="input w-56 py-2" :placeholder="$t('media.searchPlaceholder')" >
      <div class="ml-auto flex items-center gap-2 text-sm">
        <template v-if="selectMode">
          <span class="font-semibold">{{ $t('media.selected', { n: selected.size }) }}</span>
          <button class="btn-ghost btn-sm" @click="toggleAll">{{ allSelected ? $t('media.library.clear') : $t('media.library.selectAll') }}</button>
          <button class="btn-sm btn bg-brand-700 text-white hover:bg-brand-600" @click="bulkDelete">{{ $t('common.delete') }}</button>
        </template>
        <button v-else-if="items.length" class="btn-ghost btn-sm" @click="toggleAll">{{ $t('media.library.select') }}</button>
      </div>
    </div>
    <p v-if="error" class="mt-2 text-sm text-brand-700">{{ error }}</p>

    <div v-if="status === 'pending' && !items.length" class="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-4 lg:grid-cols-6">
      <div v-for="i in 12" :key="i" class="aspect-square animate-pulse rounded-xl bg-line/60" />
    </div>
    <div v-else-if="items.length" class="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-4 lg:grid-cols-6">
      <div v-for="a in items" :key="a.id" class="group">
        <button type="button" class="block w-full text-left" @click="openDetail(a)">
          <MediaTile :asset="a" :selected="selected.has(a.id)" class="aspect-square transition group-hover:shadow-lg">
            <span
              class="absolute right-1.5 top-1.5 grid size-6 place-items-center rounded-full border-2 text-xs transition"
              :class="selected.has(a.id) ? 'border-brand-500 bg-brand-500 text-white' : 'border-white bg-ink/30 text-transparent opacity-0 group-hover:opacity-100'"
              role="checkbox"
              :aria-checked="selected.has(a.id)"
              @click.stop="toggleSel(a)"
            >✓</span>
            <span v-if="a.product_count" class="absolute bottom-1.5 right-1.5 rounded-full bg-white/95 px-1.5 text-[10px] font-bold">{{ $t('media.library.productCount', { n: a.product_count }, a.product_count) }}</span>
          </MediaTile>
        </button>
        <div class="mt-1.5 truncate text-xs font-medium">{{ a.alt || a.original_name || $t('media.untitled') }}</div>
        <div class="text-[11px] text-muted">
          {{ a.source === 'url' ? $t('media.library.externalLink') : bytes(a.size_bytes) }}<template v-if="a.width"> · {{ a.width }}×{{ a.height }}</template>
        </div>
      </div>
    </div>
    <EmptyState v-else class="mt-4" :title="filter === 'unused' ? $t('media.library.emptyUnusedTitle') : $t('media.library.emptyTitle')" :text="filter === 'unused' ? $t('media.library.emptyUnusedText') : $t('media.library.emptyText')" />

    <div v-if="notice" class="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-full bg-ink px-4 py-2 text-sm font-semibold text-white shadow-lg">{{ notice }}</div>

    <!-- Detail drawer -->
    <Teleport to="body">
      <div v-if="active" class="fixed inset-0 z-40 bg-ink/30" @click="active = null" />
      <aside class="fixed inset-y-0 right-0 z-50 flex w-full max-w-md flex-col bg-white shadow-2xl transition-transform" :class="active ? 'translate-x-0' : 'translate-x-full'">
        <template v-if="active">
          <div class="flex items-center border-b border-line px-5 py-3">
            <div class="truncate font-bold">{{ active.original_name || (active.kind === 'video' ? $t('media.video') : $t('media.image')) }}</div>
            <button class="ml-auto px-2 text-muted hover:text-ink" :aria-label="$t('common.close')" @click="active = null">✕</button>
          </div>
          <div class="flex-1 space-y-5 overflow-y-auto p-5">
            <div class="overflow-hidden rounded-2xl bg-paper">
              <video v-if="active.kind === 'video'" :src="active.url" :poster="active.thumb_url ?? undefined" controls playsinline class="max-h-80 w-full bg-ink" />
              <img v-else :src="active.url" :alt="active.alt" class="max-h-80 w-full object-contain" >
            </div>
            <form class="space-y-2" @submit.prevent="saveAlt">
              <label class="label">{{ $t('media.altText') }}</label>
              <div class="flex gap-2"><input v-model="alt" maxlength="300" class="input" :placeholder="$t('media.library.altPlaceholder')"><button class="btn-dark btn-sm">{{ $t('common.save') }}</button></div>
            </form>
            <dl class="grid grid-cols-2 gap-3 text-sm">
              <div><dt class="text-xs text-muted">{{ $t('media.library.type') }}</dt><dd class="font-medium">{{ active.mime || active.kind }}</dd></div>
              <div><dt class="text-xs text-muted">{{ $t('media.library.size') }}</dt><dd class="font-medium">{{ active.source === 'url' ? $t('media.library.external') : bytes(active.size_bytes) }}</dd></div>
              <div v-if="active.width"><dt class="text-xs text-muted">{{ $t('media.library.dimensions') }}</dt><dd class="font-medium">{{ active.width }} × {{ active.height }}</dd></div>
              <div><dt class="text-xs text-muted">{{ $t('media.library.added') }}</dt><dd class="font-medium">{{ fmtDate(active.created_at) }}</dd></div>
            </dl>
            <div>
              <label class="label">URL</label>
              <div class="flex gap-2"><input :value="active.url" readonly class="input bg-paper font-mono text-xs" @focus="($event.target as HTMLInputElement).select()"><button class="btn-ghost btn-sm" @click="copy(active.url)">{{ $t('common.copy') }}</button></div>
            </div>
            <div>
              <label class="label">{{ $t('media.library.usedIn') }}</label>
              <p v-if="!detail" class="text-sm text-muted">{{ $t('common.loading') }}</p>
              <ul v-else-if="detail.products.length" class="space-y-1 text-sm">
                <li v-for="p in detail.products" :key="p.id">
                  <NuxtLink :to="`/dashboard/products/${p.id}`" class="font-medium hover:underline">{{ p.name }}</NuxtLink>
                  <span v-if="p.is_cover" class="chip ml-1 bg-ink text-white">{{ $t('media.cover') }}</span>
                </li>
              </ul>
              <p v-else class="text-sm text-muted">{{ $t('media.library.notUsed') }}</p>
            </div>
          </div>
          <div class="border-t border-line p-4">
            <button class="btn w-full bg-brand-50 text-brand-700 hover:bg-brand-100" @click="deleteActive">{{ $t('media.library.deleteFromLibrary') }}</button>
          </div>
        </template>
      </aside>
    </Teleport>
  </div>
</template>
