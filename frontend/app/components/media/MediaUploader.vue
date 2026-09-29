<script setup lang="ts">
import type { MediaAsset } from '~/utils/types'
import { ACCEPT, type UploadJob } from '~/composables/useMedia'

const props = withDefaults(defineProps<{ shopId: string; productId?: string; compact?: boolean; hint?: string }>(), {
  productId: undefined,
  hint: undefined,
})
const emit = defineEmits<{ uploaded: [assets: MediaAsset[]] }>()
const { makeJobs, runJobs, checkResolution } = useMedia()
const { data: cfg } = useMediaConfig()
const jobs = ref<UploadJob[]>([])
const over = ref(false)
const input = ref<HTMLInputElement | null>(null)
const busy = computed(() => jobs.value.some((j) => j.status === 'uploading' || j.status === 'queued'))

async function add(files: FileList | File[] | null | undefined) {
  if (!files || !files.length) return
  const fresh = reactive(makeJobs(files)) as UploadJob[]
  jobs.value.push(...fresh)
  await checkResolution(fresh, cfg.value.min_image_edge)
  const assets = await runJobs(props.shopId, fresh, props.productId)
  if (assets.length) emit('uploaded', assets)
  // clear finished rows after a moment, keep errors visible
  setTimeout(() => {
    jobs.value = jobs.value.filter((j) => j.status === 'error' || j.status === 'uploading' || j.status === 'queued')
  }, 1500)
}

function onDrop(e: DragEvent) {
  over.value = false
  add(e.dataTransfer?.files)
}
function onPaste(e: ClipboardEvent) {
  const files = Array.from(e.clipboardData?.files ?? [])
  if (files.length) add(files)
}
function dismiss(id: string) {
  jobs.value = jobs.value.filter((j) => j.id !== id)
}
defineExpose({ add, busy })
</script>

<template>
  <div @paste="onPaste">
    <button
      type="button"
      class="flex w-full flex-col items-center justify-center rounded-2xl border-2 border-dashed text-center transition"
      :class="[over ? 'border-brand-500 bg-brand-50' : 'border-line bg-paper hover:border-ink/40', compact ? 'gap-1 px-4 py-5' : 'gap-2 px-6 py-10']"
      @click="input?.click()"
      @dragover.prevent="over = true"
      @dragleave.prevent="over = false"
      @drop.prevent="onDrop"
    >
      <span class="grid size-10 place-items-center rounded-full bg-surface text-lg shadow-sm">⇪</span>
      <i18n-t keypath="media.uploader.drop" tag="span" class="text-sm font-semibold"><template #browse><span class="text-brand-600 underline">{{ $t('media.uploader.browse') }}</span></template></i18n-t>
      <span v-if="!compact" class="text-xs text-muted">{{ hint ?? $t('media.uploader.hint') }}</span>
      <span v-if="!compact" class="max-w-md text-[11px] text-muted/80">{{ $t('media.quality.hint', { min: cfg.min_image_edge, rec: cfg.recommended_edge }) }}</span>
    </button>
    <input ref="input" type="file" :accept="ACCEPT" multiple class="hidden" @change="add(($event.target as HTMLInputElement).files); ($event.target as HTMLInputElement).value = ''" >

    <ul v-if="jobs.length" class="mt-3 space-y-2">
      <li v-for="j in jobs" :key="j.id" class="flex items-center gap-3 rounded-xl border border-line bg-surface p-2 text-sm">
        <div class="size-10 shrink-0 overflow-hidden rounded-lg bg-line/60">
          <img v-if="j.preview" :src="j.preview" class="size-full object-cover" alt="">
          <div v-else class="grid size-full place-items-center text-xs">▶</div>
        </div>
        <div class="min-w-0 flex-1">
          <div class="flex justify-between gap-2">
            <span class="truncate font-medium">{{ j.file.name }}</span>
            <span class="shrink-0 text-xs text-muted">{{ bytes(j.file.size) }}</span>
          </div>
          <div v-if="j.status === 'error'" class="text-xs text-brand-700">{{ j.error }}</div>
          <div v-else class="mt-1 h-1.5 overflow-hidden rounded-full bg-line">
            <div class="h-full rounded-full transition-all" :class="j.status === 'done' ? 'bg-mint-500' : 'bg-brand-500'" :style="{ width: `${j.status === 'queued' ? 2 : j.progress}%` }" />
          </div>
        </div>
        <button v-if="j.status === 'error'" type="button" class="px-1 text-muted hover:text-ink" :aria-label="$t('media.dismiss')" @click="dismiss(j.id)">✕</button>
        <span v-else-if="j.status === 'done'" class="text-mint-500">✓</span>
      </li>
    </ul>
  </div>
</template>
