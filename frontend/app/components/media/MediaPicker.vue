<script setup lang="ts">
import type { MediaAsset } from '~/utils/types'

/** Modal to choose assets from the shop library, upload new ones, or add by URL. */
const props = withDefaults(
  defineProps<{ shopId: string; multiple?: boolean; kind?: 'image' | 'video' | ''; max?: number; exclude?: string[] }>(),
  { multiple: true, kind: '', max: 15, exclude: () => [] },
)
const open = defineModel<boolean>({ default: false })
const emit = defineEmits<{ select: [assets: MediaAsset[]] }>()
const api = useApi()
const { addUrl } = useMedia()

const tab = ref<'library' | 'upload' | 'url'>('library')
const filter = ref<'' | 'image' | 'video'>(props.kind)
const q = ref('')
const items = ref<MediaAsset[]>([])
const loading = ref(false)
const picked = ref<MediaAsset[]>([])
const error = ref('')

async function load() {
  loading.value = true
  try {
    const r = await api<{ items: MediaAsset[] }>(`/shops/${props.shopId}/media`, {
      query: { kind: filter.value || undefined, q: q.value || undefined, limit: 120 },
    })
    items.value = r.items.filter((a) => !props.exclude.includes(a.id))
  } catch (e) {
    error.value = apiError(e)
  } finally {
    loading.value = false
  }
}
watch(open, (o) => {
  if (o) {
    picked.value = []
    error.value = ''
    tab.value = 'library'
    load()
  }
})
watch(filter, load)
let t: ReturnType<typeof setTimeout> | undefined
watch(q, () => { clearTimeout(t); t = setTimeout(load, 250) })

const isPicked = (a: MediaAsset) => picked.value.some((p) => p.id === a.id)
function toggle(a: MediaAsset) {
  if (!props.multiple) {
    picked.value = [a]
    return confirm()
  }
  if (isPicked(a)) picked.value = picked.value.filter((p) => p.id !== a.id)
  else if (picked.value.length < props.max) picked.value.push(a)
}
function confirm() {
  emit('select', picked.value)
  open.value = false
}
function onUploaded(assets: MediaAsset[]) {
  const usable = assets.filter((a) => !props.kind || a.kind === props.kind)
  items.value = [...usable, ...items.value]
  if (!props.multiple && usable[0]) {
    picked.value = [usable[0]]
    return confirm()
  }
  for (const a of usable) if (picked.value.length < props.max && !isPicked(a)) picked.value.push(a)
  tab.value = 'library'
}
const url = ref('')
async function submitUrl() {
  error.value = ''
  try {
    const a = await addUrl(props.shopId, url.value)
    url.value = ''
    onUploaded([a])
  } catch (e) {
    error.value = apiError(e)
  }
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') open.value = false
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="fixed inset-0 z-50 grid place-items-center bg-ink/40 p-3 sm:p-6" @click.self="open = false" @keydown="onKey">
      <div class="card flex max-h-[90dvh] w-full max-w-4xl flex-col overflow-hidden shadow-2xl" role="dialog" :aria-label="$t('media.title')">
        <div class="flex items-center gap-2 border-b border-line px-5 py-3">
          <div class="font-bold">{{ $t('media.title') }}</div>
          <div class="ml-4 flex rounded-full bg-paper p-1 text-xs font-semibold">
            <button v-for="t in (['library', 'upload', 'url'] as const)" :key="t" type="button" class="rounded-full px-3 py-1" :class="tab === t && 'bg-ink text-white'" @click="tab = t">
              {{ $t(`media.picker.tab.${t}`) }}
            </button>
          </div>
          <button type="button" class="ml-auto px-2 text-muted hover:text-ink" :aria-label="$t('common.close')" @click="open = false">✕</button>
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto p-5">
          <p v-if="error" class="mb-3 text-sm text-brand-700">{{ error }}</p>
          <template v-if="tab === 'library'">
            <div class="mb-4 flex flex-wrap gap-2">
              <input v-model="q" class="input max-w-xs py-2" :placeholder="$t('media.searchPlaceholder')" >
              <select v-if="!kind" v-model="filter" class="input w-36 py-2"><option value="">{{ $t('media.allMedia') }}</option><option value="image">{{ $t('media.images') }}</option><option value="video">{{ $t('media.videos') }}</option></select>
            </div>
            <div v-if="loading && !items.length" class="grid grid-cols-3 gap-3 sm:grid-cols-5">
              <div v-for="i in 10" :key="i" class="aspect-square animate-pulse rounded-xl bg-line/60" />
            </div>
            <div v-else-if="items.length" class="grid grid-cols-3 gap-3 sm:grid-cols-5">
              <button v-for="a in items" :key="a.id" type="button" class="group text-left" @click="toggle(a)">
                <MediaTile :asset="a" :selected="isPicked(a)" class="aspect-square">
                  <span v-if="isPicked(a) && multiple" class="absolute right-1.5 top-1.5 grid size-6 place-items-center rounded-full bg-brand-500 text-xs font-bold text-white">
                    {{ picked.findIndex((p) => p.id === a.id) + 1 }}
                  </span>
                </MediaTile>
                <div class="mt-1 truncate text-[11px] text-muted">{{ a.alt || a.original_name || '—' }}</div>
              </button>
            </div>
            <EmptyState v-else :title="$t('media.picker.emptyTitle')" :text="$t('media.picker.emptyText')">
              <button type="button" class="btn-primary" @click="tab = 'upload'">{{ $t('media.picker.uploadMedia') }}</button>
            </EmptyState>
          </template>
          <MediaUploader v-else-if="tab === 'upload'" :shop-id="shopId" @uploaded="onUploaded" />
          <form v-else class="space-y-3" @submit.prevent="submitUrl">
            <p class="text-sm text-muted">{{ $t('media.picker.urlHelp') }}</p>
            <input v-model="url" type="url" required class="input" placeholder="https://…/photo.jpg" >
            <button class="btn-dark">{{ $t('media.picker.addToLibrary') }}</button>
          </form>
        </div>

        <div v-if="multiple" class="flex items-center gap-3 border-t border-line px-5 py-3">
          <span class="text-sm text-muted">{{ $t('media.selected', { n: picked.length }) }}{{ max < 100 ? ` · ${$t('media.picker.max', { max })}` : '' }}</span>
          <button type="button" class="btn-ghost btn-sm ml-auto" @click="open = false">{{ $t('common.cancel') }}</button>
          <button type="button" class="btn-primary btn-sm" :disabled="!picked.length" @click="confirm">{{ $t('media.picker.addN', { n: picked.length || '' }) }}</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
