<script setup lang="ts">
import type { MediaVariant } from '~/utils/types'

definePageMeta({ layout: 'admin', middleware: 'admin' })
const api = useApi()
const { t } = useI18n()

interface Stock {
  id: string
  title: string
  keywords: string
  brand: string
  credit: string
  url: string
  thumb_url: string | null
  width: number | null
  height: number | null
  variants: MediaVariant[]
  placeholder: string
  dominant_color: string
  active: boolean
  use_count: number
  source_asset_id: string | null
  created_at: string
}
interface Candidate { asset_id: string; url: string; thumb_url: string | null; variants: MediaVariant[]; placeholder: string; width: number | null; height: number | null; product: string; shop: string; alt: string }
interface Synonym { id: number; terms: string[] }

type Tab = 'library' | 'promote' | 'synonyms'
const tab = ref<Tab>('library')
const q = ref('')
const search = ref('')
const { data, refresh } = await useAsyncData('stock-images', () => api<{ items: Stock[]; total: number; picks: number; sharing_shops: number }>('/admin/image-suggest/stock', { query: { q: search.value || undefined } }), {
  watch: [search],
  default: () => ({ items: [] as Stock[], total: 0, picks: 0, sharing_shops: 0 }),
})
const msg = ref('')
const error = ref('')
const busy = ref(false)
function done(m: string) {
  msg.value = m
  error.value = ''
}

// ---- upload
const up = reactive<{ file: File | null; title: string; keywords: string; brand: string; credit: string }>({ file: null, title: '', keywords: '', brand: '', credit: '' })
const preview = computed(() => (up.file && import.meta.client ? URL.createObjectURL(up.file) : ''))
const fileInput = ref<HTMLInputElement | null>(null)
function onFile(e: Event) {
  const f = (e.target as HTMLInputElement).files?.[0] ?? null
  up.file = f
  if (f && !up.title) up.title = f.name.replace(/\.[^.]+$/, '').replace(/[_-]+/g, ' ')
}
async function upload() {
  if (!up.file) return
  busy.value = true
  error.value = msg.value = ''
  try {
    const fd = new FormData()
    fd.append('title', up.title)
    fd.append('keywords', up.keywords)
    fd.append('brand', up.brand)
    fd.append('credit', up.credit)
    fd.append('file', up.file)
    await api('/admin/image-suggest/stock', { method: 'POST', body: fd })
    Object.assign(up, { file: null, title: '', keywords: '', brand: '', credit: '' })
    if (fileInput.value) fileInput.value.value = ''
    done(t('imgsuggest.admin.uploaded'))
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}

// ---- edit
const editing = ref<Stock | null>(null)
const ed = reactive({ title: '', keywords: '', brand: '', credit: '' })
function edit(s: Stock) {
  editing.value = s
  Object.assign(ed, { title: s.title, keywords: s.keywords, brand: s.brand, credit: s.credit })
}
async function patch(s: Stock, body: Record<string, unknown>) {
  error.value = msg.value = ''
  try {
    await api(`/admin/image-suggest/stock/${s.id}`, { method: 'PATCH', body })
    editing.value = null
    done(t('common.saved'))
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
async function remove(s: Stock) {
  if (!confirm(t('imgsuggest.admin.deleteConfirm', { title: s.title }))) return
  try {
    await api(`/admin/image-suggest/stock/${s.id}`, { method: 'DELETE' })
    done(t('imgsuggest.admin.deleted'))
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}

// ---- promote shop photos
const pq = ref('')
const candidates = ref<Candidate[]>([])
const promoting = ref<Candidate | null>(null)
const pm = reactive({ title: '', keywords: '', brand: '', credit: '' })
async function findCandidates() {
  error.value = ''
  candidates.value = (await api<{ items: Candidate[] }>('/admin/image-suggest/candidates', { query: { q: pq.value } }).catch((e) => {
    error.value = apiError(e)
    return { items: [] }
  })).items
}
function startPromote(c: Candidate) {
  promoting.value = c
  Object.assign(pm, { title: c.product, keywords: '', brand: '', credit: t('imgsuggest.admin.creditFrom', { shop: c.shop }) })
}
async function promote() {
  if (!promoting.value) return
  busy.value = true
  error.value = msg.value = ''
  try {
    await api('/admin/image-suggest/stock/promote', { method: 'POST', body: { asset_id: promoting.value.asset_id, ...pm } })
    candidates.value = candidates.value.filter((c) => c.asset_id !== promoting.value?.asset_id)
    promoting.value = null
    done(t('imgsuggest.admin.promoted'))
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}

// ---- synonyms
const { data: synonyms, refresh: refreshSyn } = await useLazyAsyncData('stock-synonyms', () => api<Synonym[]>('/admin/image-suggest/synonyms'), { default: () => [] as Synonym[] })
const newSyn = ref('')
async function addSyn() {
  error.value = msg.value = ''
  try {
    await api('/admin/image-suggest/synonyms', { method: 'POST', body: { terms: newSyn.value.split(/[,\n]+/).map((x) => x.trim()).filter(Boolean) } })
    newSyn.value = ''
    await refreshSyn()
  } catch (e) {
    error.value = apiError(e)
  }
}
async function delSyn(s: Synonym) {
  await api(`/admin/image-suggest/synonyms/${s.id}`, { method: 'DELETE' }).catch((e) => (error.value = apiError(e)))
  await refreshSyn()
}
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('imgsuggest.admin.title') }}</h1>
    <p class="max-w-3xl text-sm text-muted">{{ $t('imgsuggest.admin.subtitle') }}</p>
    <div class="mt-4 flex flex-wrap gap-5 text-sm">
      <div><span class="text-lg font-extrabold">{{ num(data.total) }}</span> <span class="text-muted">{{ $t('imgsuggest.admin.statPhotos') }}</span></div>
      <div><span class="text-lg font-extrabold">{{ num(data.picks) }}</span> <span class="text-muted">{{ $t('imgsuggest.admin.statPicks') }}</span></div>
      <div><span class="text-lg font-extrabold">{{ num(data.sharing_shops) }}</span> <span class="text-muted">{{ $t('imgsuggest.admin.statSharing') }}</span></div>
    </div>

    <div class="mt-5 flex flex-wrap items-center gap-3">
      <div class="flex flex-wrap rounded-2xl bg-surface p-1 text-sm font-semibold shadow-card">
        <button v-for="s in (['library', 'promote', 'synonyms'] as Tab[])" :key="s" class="rounded-xl px-3.5 py-1.5" :class="tab === s && 'bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]'" @click="tab = s">{{ $t(`imgsuggest.admin.tabs.${s}`) }}</button>
      </div>
      <form v-if="tab === 'library'" class="ml-auto" @submit.prevent="search = q.trim()">
        <UInput v-model="q" icon="i-lucide-search" class="w-64" :placeholder="$t('imgsuggest.admin.search')" :aria-label="$t('imgsuggest.admin.search')" />
      </form>
    </div>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>
    <p v-if="msg" class="mt-3 text-sm text-mint-500">{{ msg }}</p>

    <!-- Library -->
    <section v-if="tab === 'library'" class="mt-4 grid gap-5 lg:grid-cols-[20rem_1fr]">
      <form class="card space-y-3 self-start p-4" @submit.prevent="upload">
        <h2 class="font-bold">{{ $t('imgsuggest.admin.add') }}</h2>
        <label class="grid aspect-video cursor-pointer place-items-center overflow-hidden rounded-xl border-2 border-dashed border-line bg-paper text-sm text-muted hover:border-ink/40">
          <img v-if="preview" :src="preview" alt="" class="size-full object-contain">
          <span v-else class="text-center"><UIcon name="i-lucide-image-plus" class="mx-auto size-7" /><span class="block">{{ $t('imgsuggest.admin.choose') }}</span></span>
          <input ref="fileInput" type="file" accept="image/jpeg,image/png,image/webp" class="sr-only" @change="onFile">
        </label>
        <div><label class="label" for="st-title">{{ $t('imgsuggest.admin.fTitle') }}</label><UInput id="st-title" v-model="up.title" class="w-full" required maxlength="200" :placeholder="$t('imgsuggest.admin.titlePh')" /></div>
        <div><label class="label" for="st-kw">{{ $t('imgsuggest.admin.fKeywords') }}</label><UTextarea id="st-kw" v-model="up.keywords" class="w-full" :rows="2" :placeholder="$t('imgsuggest.admin.keywordsPh')" /></div>
        <div class="grid grid-cols-2 gap-2">
          <div><label class="label" for="st-brand">{{ $t('imgsuggest.admin.fBrand') }}</label><UInput id="st-brand" v-model="up.brand" class="w-full" maxlength="100" /></div>
          <div><label class="label" for="st-credit">{{ $t('imgsuggest.admin.fCredit') }}</label><UInput id="st-credit" v-model="up.credit" class="w-full" maxlength="300" /></div>
        </div>
        <p class="text-xs text-muted">{{ $t('imgsuggest.admin.rights') }}</p>
        <UButton type="submit" block icon="i-lucide-upload" :loading="busy" :disabled="!up.file || up.title.trim().length < 2">{{ $t('imgsuggest.admin.upload') }}</UButton>
      </form>

      <div>
        <div v-if="data.items.length" class="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-4">
          <div v-for="s in data.items" :key="s.id" class="card overflow-hidden" :class="!s.active && 'opacity-60'">
            <AppImage :src="s.thumb_url || s.url" :alt="s.title" :variants="s.variants" :placeholder="s.placeholder" :color="s.dominant_color" :width="s.width" :height="s.height" sizes="220px" class="aspect-square w-full" />
            <div class="space-y-1 p-3">
              <div class="truncate font-semibold" :title="s.title">{{ s.title }}</div>
              <div class="truncate text-xs text-muted" :title="s.keywords">{{ [s.brand, s.keywords].filter(Boolean).join(' · ') || '—' }}</div>
              <div class="flex items-center justify-between text-[11px] text-muted">
                <span>{{ $t('imgsuggest.admin.used', { n: s.use_count }, s.use_count) }}</span>
                <UBadge v-if="!s.active" color="neutral" variant="soft" size="sm">{{ $t('imgsuggest.admin.hidden') }}</UBadge>
              </div>
              <div class="flex gap-1 pt-1">
                <UButton size="xs" color="neutral" variant="soft" icon="i-lucide-pencil" @click="edit(s)">{{ $t('common.edit') }}</UButton>
                <UButton size="xs" color="neutral" variant="ghost" :icon="s.active ? 'i-lucide-eye-off' : 'i-lucide-eye'" :aria-label="s.active ? $t('imgsuggest.admin.hide') : $t('imgsuggest.admin.show')" @click="patch(s, { active: !s.active })" />
                <UButton size="xs" color="error" variant="ghost" icon="i-lucide-trash-2" class="ml-auto" :aria-label="$t('common.delete')" @click="remove(s)" />
              </div>
            </div>
          </div>
        </div>
        <EmptyState v-else icon="i-lucide-images" :title="$t('imgsuggest.admin.empty')" :text="$t('imgsuggest.admin.emptyText')" />
      </div>
    </section>

    <!-- Promote -->
    <section v-if="tab === 'promote'" class="mt-4 space-y-4">
      <p class="max-w-3xl text-sm text-muted">{{ $t('imgsuggest.admin.promoteIntro') }}</p>
      <form class="flex max-w-lg gap-2" @submit.prevent="findCandidates">
        <UInput v-model="pq" class="flex-1" icon="i-lucide-search" :placeholder="$t('imgsuggest.admin.promoteSearch')" :aria-label="$t('imgsuggest.admin.promoteSearch')" />
        <UButton type="submit" :disabled="pq.trim().length < 2">{{ $t('common.search') }}</UButton>
      </form>
      <div v-if="candidates.length" class="grid grid-cols-2 gap-3 sm:grid-cols-4 xl:grid-cols-6">
        <button v-for="c in candidates" :key="c.asset_id" type="button" class="card overflow-hidden text-left transition hover:ring-2 hover:ring-brand-500" @click="startPromote(c)">
          <AppImage :src="c.thumb_url || c.url" :alt="c.product" :variants="c.variants" :placeholder="c.placeholder" :width="c.width" :height="c.height" sizes="180px" class="aspect-square w-full" />
          <div class="p-2"><div class="truncate text-xs font-semibold">{{ c.product }}</div><div class="truncate text-[11px] text-muted">{{ c.shop }}</div></div>
        </button>
      </div>
    </section>

    <!-- Synonyms -->
    <section v-if="tab === 'synonyms'" class="mt-4 max-w-3xl space-y-4">
      <p class="text-sm text-muted">{{ $t('imgsuggest.admin.synIntro') }}</p>
      <form class="flex gap-2" @submit.prevent="addSyn">
        <UInput v-model="newSyn" class="flex-1" :placeholder="$t('imgsuggest.admin.synPh')" :aria-label="$t('imgsuggest.admin.synPh')" />
        <UButton type="submit" icon="i-lucide-plus" :disabled="!newSyn.includes(',')">{{ $t('common.add') }}</UButton>
      </form>
      <div class="card divide-y divide-line/70">
        <div v-for="s in synonyms" :key="s.id" class="flex items-center gap-2 px-4 py-2.5">
          <div class="flex flex-1 flex-wrap gap-1"><span v-for="w in s.terms" :key="w" class="chip bg-[var(--field)]">{{ w }}</span></div>
          <UButton size="xs" color="neutral" variant="ghost" icon="i-lucide-x" :aria-label="$t('common.delete')" @click="delSyn(s)" />
        </div>
      </div>
    </section>

    <UModal :open="!!editing" :title="$t('common.edit')" @update:open="(o) => !o && (editing = null)">
      <template #body>
        <form id="st-edit" class="space-y-3" @submit.prevent="editing && patch(editing, { ...ed })">
          <div><label class="label">{{ $t('imgsuggest.admin.fTitle') }}</label><UInput v-model="ed.title" class="w-full" /></div>
          <div><label class="label">{{ $t('imgsuggest.admin.fKeywords') }}</label><UTextarea v-model="ed.keywords" class="w-full" :rows="2" /></div>
          <div class="grid grid-cols-2 gap-2">
            <div><label class="label">{{ $t('imgsuggest.admin.fBrand') }}</label><UInput v-model="ed.brand" class="w-full" /></div>
            <div><label class="label">{{ $t('imgsuggest.admin.fCredit') }}</label><UInput v-model="ed.credit" class="w-full" /></div>
          </div>
        </form>
      </template>
      <template #footer>
        <div class="flex w-full justify-end gap-2"><UButton color="neutral" variant="soft" @click="editing = null">{{ $t('common.cancel') }}</UButton><UButton type="submit" form="st-edit">{{ $t('common.save') }}</UButton></div>
      </template>
    </UModal>

    <UModal :open="!!promoting" :title="$t('imgsuggest.admin.promoteTitle')" @update:open="(o) => !o && (promoting = null)">
      <template #body>
        <form v-if="promoting" id="st-promote" class="space-y-3" @submit.prevent="promote">
          <div class="flex gap-3">
            <AppImage :src="promoting.thumb_url || promoting.url" :alt="promoting.product" :variants="promoting.variants" :placeholder="promoting.placeholder" sizes="96px" class="size-24 shrink-0 rounded-xl" />
            <p class="text-xs text-muted">{{ $t('imgsuggest.admin.promoteHelp', { shop: promoting.shop }) }}</p>
          </div>
          <div><label class="label">{{ $t('imgsuggest.admin.fTitle') }}</label><UInput v-model="pm.title" class="w-full" required /></div>
          <div><label class="label">{{ $t('imgsuggest.admin.fKeywords') }}</label><UTextarea v-model="pm.keywords" class="w-full" :rows="2" :placeholder="$t('imgsuggest.admin.keywordsPh')" /></div>
          <div class="grid grid-cols-2 gap-2">
            <div><label class="label">{{ $t('imgsuggest.admin.fBrand') }}</label><UInput v-model="pm.brand" class="w-full" /></div>
            <div><label class="label">{{ $t('imgsuggest.admin.fCredit') }}</label><UInput v-model="pm.credit" class="w-full" /></div>
          </div>
        </form>
      </template>
      <template #footer>
        <div class="flex w-full justify-end gap-2"><UButton color="neutral" variant="soft" @click="promoting = null">{{ $t('common.cancel') }}</UButton><UButton type="submit" form="st-promote" :loading="busy" icon="i-lucide-arrow-up-to-line">{{ $t('imgsuggest.admin.promote') }}</UButton></div>
      </template>
    </UModal>
  </div>
</template>
