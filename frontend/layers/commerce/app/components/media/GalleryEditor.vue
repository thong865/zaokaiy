<script setup lang="ts">
import type { MediaAsset } from '~/utils/types'

/**
 * Product gallery editor: drag to reorder (first = cover), add from library / upload / URL,
 * edit alt text, remove. With `productId` every change is saved immediately; without it
 * (new product) the ordered list is exposed through v-model for the parent to submit.
 */
const props = defineProps<{
  shopId: string
  productId?: string
  /** Product name being typed — plug-in modules may suggest matching photos for it. */
  suggestFor?: string
  categoryId?: string | null
}>()
const items = defineModel<MediaAsset[]>({ default: () => [] })
const emit = defineEmits<{ saved: [] }>()
const { setGallery } = useMedia()
const api = useApi()
const { t } = useI18n()
const MAX = 15

const picker = ref(false)
const saving = ref(false)
const status = ref('')
const error = ref('')
const editing = ref<MediaAsset | null>(null)
const altDraft = ref('')

async function persist(next: MediaAsset[]) {
  const prev = items.value
  items.value = next
  if (!props.productId) return
  saving.value = true
  error.value = ''
  try {
    items.value = await setGallery(props.productId, next.map((a) => a.id))
    status.value = 'saved'
    emit('saved')
    setTimeout(() => (status.value = ''), 1500)
  } catch (e) {
    items.value = prev
    error.value = apiError(e)
  } finally {
    saving.value = false
  }
}

function addAssets(assets: MediaAsset[]) {
  const fresh = assets.filter((a) => !items.value.some((i) => i.id === a.id))
  const room = MAX - items.value.length
  if (fresh.length > room) error.value = t('media.gallery.tooMany', { max: MAX, n: fresh.length - room })
  persist([...items.value, ...fresh.slice(0, room)])
}
const remove = (a: MediaAsset) => persist(items.value.filter((i) => i.id !== a.id))
const makeCover = (a: MediaAsset) => persist([a, ...items.value.filter((i) => i.id !== a.id)])
function move(i: number, dir: -1 | 1) {
  const j = i + dir
  if (j < 0 || j >= items.value.length) return
  const next = [...items.value]
  ;[next[i], next[j]] = [next[j]!, next[i]!]
  persist(next)
}

// Native drag & drop reordering
const dragFrom = ref<number | null>(null)
const dragOver = ref<number | null>(null)
function onDrop(to: number) {
  const from = dragFrom.value
  dragFrom.value = dragOver.value = null
  if (from === null || from === to) return
  const next = [...items.value]
  const [moved] = next.splice(from, 1)
  next.splice(to, 0, moved!)
  persist(next)
}

function openAlt(a: MediaAsset) {
  editing.value = a
  altDraft.value = a.alt
}
async function saveAlt() {
  if (!editing.value) return
  try {
    const updated = await api<MediaAsset>(`/media/${editing.value.id}`, { method: 'PATCH', body: { alt: altDraft.value } })
    items.value = items.value.map((i) => (i.id === updated.id ? { ...i, alt: updated.alt } : i))
    editing.value = null
  } catch (e) {
    error.value = apiError(e)
  }
}
</script>

<template>
  <div>
    <div class="flex items-center justify-between gap-2">
      <label class="label mb-0">{{ $t('media.gallery.title') }} <span class="normal-case tracking-normal">({{ items.length }}/{{ MAX }})</span></label>
      <span class="text-xs" :class="error ? 'text-brand-700' : 'text-muted'">{{ saving ? $t('common.saving') : status ? $t('common.saved') : '' }}</span>
    </div>
    <p class="mb-3 mt-1 text-xs text-muted">{{ $t('media.gallery.hint') }}</p>

    <div class="grid grid-cols-3 gap-3 sm:grid-cols-4 lg:grid-cols-5">
      <div
        v-for="(a, i) in items"
        :key="a.id"
        class="group relative"
        draggable="true"
        :class="[dragOver === i && dragFrom !== i && 'scale-[1.03]', dragFrom === i && 'opacity-40']"
        @dragstart="dragFrom = i"
        @dragend="dragFrom = dragOver = null"
        @dragover.prevent="dragOver = i"
        @drop.prevent="onDrop(i)"
      >
        <MediaTile :asset="a" show-quality sizes="(min-width: 1024px) 160px, 30vw" class="aspect-square cursor-grab transition active:cursor-grabbing" :class="dragOver === i && dragFrom !== i && 'ring-2 ring-brand-500'">
          <span v-if="i === 0" class="absolute left-1.5 top-1.5 rounded-full bg-ink px-2 py-0.5 text-[10px] font-bold text-paper">{{ $t('media.cover') }}</span>
          <div class="absolute inset-x-1.5 bottom-1.5 flex justify-end gap-1 opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100">
            <button v-if="i > 0" type="button" class="rounded-full bg-surface/95 px-2 py-0.5 text-[10px] font-semibold shadow" :title="$t('media.gallery.makeCover')" @click="makeCover(a)">★</button>
            <button type="button" class="rounded-full bg-surface/95 px-2 py-0.5 text-[10px] font-semibold shadow" :title="$t('media.altText')" @click="openAlt(a)">{{ $t('media.gallery.alt') }}</button>
            <button type="button" class="rounded-full bg-surface/95 px-2 py-0.5 text-[10px] font-semibold text-brand-700 shadow" :title="$t('media.gallery.removeFromProduct')" @click="remove(a)">✕</button>
          </div>
        </MediaTile>
        <!-- keyboard / touch reordering -->
        <div class="mt-1 flex justify-between text-[11px] text-muted">
          <button type="button" class="px-1 hover:text-ink disabled:opacity-30" :disabled="i === 0" :aria-label="$t('media.gallery.moveLeft')" @click="move(i, -1)">←</button>
          <span class="truncate">{{ a.alt || (a.kind === 'video' ? $t('media.video') : $t('media.image')) }}</span>
          <button type="button" class="px-1 hover:text-ink disabled:opacity-30" :disabled="i === items.length - 1" :aria-label="$t('media.gallery.moveRight')" @click="move(i, 1)">→</button>
        </div>
      </div>
      <button
        v-if="items.length < MAX"
        type="button"
        class="grid aspect-square place-items-center rounded-xl border-2 border-dashed border-line bg-paper text-sm font-semibold text-muted transition hover:border-ink/40 hover:text-ink"
        @click="picker = true"
      >
        <span class="text-center"><span class="block text-2xl">＋</span>{{ $t('media.gallery.add') }}</span>
      </button>
    </div>

    <!-- plug-in modules: e.g. photo suggestions for the product name (layers/image-suggest) -->
    <ModuleSlot
      v-if="suggestFor !== undefined && items.length < MAX"
      name="gallery.suggest"
      class="mt-4"
      :shop-id="shopId"
      :query="suggestFor"
      :category-id="categoryId"
      :exclude="items.map((i) => i.id)"
      @add="(a: MediaAsset) => addAssets([a])"
    />

    <MediaUploader class="mt-4" compact :shop-id="shopId" :product-id="undefined" @uploaded="addAssets" />
    <p v-if="error" class="mt-2 text-sm text-brand-700">{{ error }}</p>

    <MediaPicker v-model="picker" :shop-id="shopId" :max="MAX - items.length" :exclude="items.map((i) => i.id)" @select="addAssets" />

    <Teleport to="body">
      <div v-if="editing" class="fixed inset-0 z-50 grid place-items-center bg-night/40 p-4" @click.self="editing = null">
        <form class="card w-full max-w-md space-y-3 p-5 shadow-2xl" @submit.prevent="saveAlt">
          <div class="flex gap-3">
            <MediaTile :asset="editing" class="size-20 shrink-0" />
            <div>
              <div class="font-bold">{{ $t('media.altText') }}</div>
              <p class="text-xs text-muted">{{ $t('media.gallery.altHelp') }}</p>
            </div>
          </div>
          <UInput v-model="altDraft" maxlength="300" autofocus />
          <div class="flex justify-end gap-2">
            <UButton color="neutral" variant="soft" size="sm" type="button" @click="editing = null">{{ $t('common.cancel') }}</UButton>
            <UButton size="sm" type="submit">{{ $t('common.save') }}</UButton>
          </div>
        </form>
      </div>
    </Teleport>
  </div>
</template>
