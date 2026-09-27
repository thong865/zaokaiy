<script setup lang="ts">
import type { MediaAsset } from '~/utils/types'

/** Single-media URL field backed by the library picker (used for content & ads). */
const props = withDefaults(defineProps<{ shopId: string; kind?: 'image' | 'video' | ''; label?: string }>(), { kind: '', label: undefined })
const url = defineModel<string>({ default: '' })
const picker = ref(false)
const isVideo = computed(() => isVideoUrl(url.value))
function pick(assets: MediaAsset[]) {
  if (assets[0]) url.value = assets[0].url
}
</script>

<template>
  <div>
    <label class="label">{{ label ?? $t('media.field.label') }}</label>
    <div class="flex items-center gap-3">
      <div class="size-16 shrink-0 overflow-hidden rounded-xl border border-line bg-paper">
        <template v-if="url">
          <video v-if="isVideo" :src="`${url}#t=0.5`" muted preload="metadata" class="size-full object-cover" />
          <img v-else :src="url" alt="" class="size-full object-cover" >
        </template>
        <div v-else class="grid size-full place-items-center text-lg text-muted">▢</div>
      </div>
      <div class="flex flex-wrap gap-2">
        <button type="button" class="btn-ghost btn-sm" @click="picker = true">{{ url ? $t('media.field.change') : $t('media.field.choose') }}</button>
        <button v-if="url" type="button" class="btn-ghost btn-sm" @click="url = ''">{{ $t('common.remove') }}</button>
      </div>
    </div>
    <MediaPicker v-model="picker" :shop-id="shopId" :multiple="false" :kind="props.kind" @select="pick" />
  </div>
</template>
