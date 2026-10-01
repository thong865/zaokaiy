<script setup lang="ts">
import type { MediaAsset, MediaVariant } from '~/utils/types'

/** Photos matching the product name being typed; one click adds a photo to the gallery. */
const props = defineProps<{ shopId: string; query?: string; categoryId?: string | null; exclude?: string[] }>()
const emit = defineEmits<{ add: [asset: MediaAsset] }>()
const api = useApi()

interface Suggestion {
  source: 'stock' | 'own' | 'shared'
  id: string
  title: string
  subtitle: string
  url: string
  thumb_url: string | null
  width: number | null
  height: number | null
  variants: MediaVariant[]
  placeholder: string
  dominant_color: string
  score: number
}

const hidden = ref(false)
const q = ref('')
let timer: ReturnType<typeof setTimeout> | undefined
watch(
  () => props.query ?? '',
  (v) => {
    clearTimeout(timer)
    timer = setTimeout(() => (q.value = v.trim()), 400)
  },
  { immediate: true },
)
onBeforeUnmount(() => clearTimeout(timer))

const { data, status } = useLazyAsyncData(
  () => `img-suggest:${props.shopId}:${q.value}`,
  () =>
    q.value.length < 2
      ? Promise.resolve({ items: [] as Suggestion[] })
      : api<{ items: Suggestion[] }>(`/shops/${props.shopId}/image-suggest`, { query: { q: q.value, limit: 12, category_id: props.categoryId || undefined } }).catch(() => ({ items: [] as Suggestion[] })),
  { server: false, watch: [q, () => props.shopId], default: () => ({ items: [] as Suggestion[] }) },
)

// source:id → the asset it became in this shop's library (to show "added")
const picked = ref<Record<string, string>>({})
const busy = ref<string | null>(null)
const error = ref('')
const key = (s: Suggestion) => `${s.source}:${s.id}`
const added = (s: Suggestion) => {
  const aid = s.source === 'own' ? s.id : picked.value[key(s)]
  return !!aid && (props.exclude ?? []).includes(aid)
}
const items = computed(() => data.value.items)

async function pick(s: Suggestion) {
  if (added(s) || busy.value) return
  busy.value = key(s)
  error.value = ''
  try {
    const a = await api<MediaAsset>(`/shops/${props.shopId}/image-suggest/pick`, { method: 'POST', body: { source: s.source, id: s.id, query: q.value } })
    picked.value = { ...picked.value, [key(s)]: a.id }
    emit('add', a)
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = null
  }
}
const badge: Record<Suggestion['source'], string> = { stock: 'bg-brand-500 text-white', own: 'bg-ink text-paper', shared: 'bg-surface/95 text-ink' }
</script>

<template>
  <section v-if="!hidden && q.length >= 2 && (items.length || status === 'pending')" class="rounded-2xl border border-dashed border-brand-500/40 bg-brand-50/40 p-3" :aria-label="$t('imgsuggest.strip.title', { q })">
    <div class="flex items-center gap-2">
      <UIcon name="i-lucide-sparkles" class="size-4 text-brand-600" />
      <div class="min-w-0 flex-1">
        <div class="truncate text-sm font-bold">{{ $t('imgsuggest.strip.title', { q }) }}</div>
        <div class="text-xs text-muted">{{ $t('imgsuggest.strip.hint') }}</div>
      </div>
      <UButton size="xs" color="neutral" variant="ghost" icon="i-lucide-x" :aria-label="$t('imgsuggest.strip.hide')" @click="hidden = true" />
    </div>
    <div v-if="status === 'pending' && !items.length" class="mt-3 flex w-0 min-w-full gap-2 overflow-hidden">
      <div v-for="i in 5" :key="i" class="size-24 shrink-0 animate-pulse rounded-xl bg-line/60" />
    </div>
    <!-- w-0 + min-w-full: the row scrolls instead of widening the form -->
    <div v-else class="mt-3 flex w-0 min-w-full snap-x gap-2 overflow-x-auto pb-1">
      <button
        v-for="s in items"
        :key="key(s)"
        type="button"
        class="group relative flex w-28 shrink-0 snap-start flex-col justify-start text-left"
        :disabled="added(s) || !!busy"
        :title="s.title"
        @click="pick(s)"
      >
        <div class="relative aspect-square overflow-hidden rounded-xl ring-2 transition" :class="added(s) ? 'ring-mint-500' : 'ring-transparent group-hover:ring-brand-500'">
          <AppImage :src="s.thumb_url || s.url" :alt="s.title" :variants="s.variants" :placeholder="s.placeholder" :color="s.dominant_color" :width="s.width" :height="s.height" sizes="112px" class="absolute inset-0 size-full" />
          <span class="absolute left-1 top-1 rounded-full px-1.5 py-0.5 text-[9px] font-bold shadow" :class="badge[s.source]">{{ $t(`imgsuggest.source.${s.source}`) }}</span>
          <span v-if="added(s)" class="absolute inset-0 grid place-items-center bg-night/45 text-xs font-bold text-white"><UIcon name="i-lucide-check" class="size-6" /></span>
          <span v-else-if="busy === key(s)" class="absolute inset-0 grid place-items-center bg-night/30"><UIcon name="i-lucide-loader-circle" class="size-6 animate-spin text-white" /></span>
          <span v-else class="absolute inset-x-1 bottom-1 rounded-lg bg-surface/95 py-0.5 text-center text-[10px] font-bold opacity-0 shadow transition group-hover:opacity-100">＋ {{ $t('imgsuggest.strip.use') }}</span>
        </div>
        <div class="mt-1 truncate text-[11px] font-semibold">{{ s.title }}</div>
        <div v-if="s.subtitle" class="truncate text-[10px] text-muted">{{ s.source === 'shared' ? $t('imgsuggest.strip.from', { shop: s.subtitle }) : s.subtitle }}</div>
      </button>
    </div>
    <p v-if="error" class="mt-2 text-xs text-brand-700">{{ error }}</p>
  </section>
</template>
