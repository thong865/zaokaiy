<script setup lang="ts">
import type { MediaAsset } from '~/utils/types'

defineProps<{ asset: Pick<MediaAsset, 'kind' | 'url' | 'thumb_url' | 'alt'>; selected?: boolean; rounded?: string }>()
const failed = ref(false)
</script>

<template>
  <div class="relative overflow-hidden bg-line/50" :class="[rounded ?? 'rounded-xl', selected && 'ring-4 ring-brand-500 ring-offset-2']">
    <img
      v-if="(asset.thumb_url || asset.kind === 'image') && !failed"
      :src="asset.thumb_url || asset.url"
      :alt="asset.alt"
      class="size-full object-cover"
      loading="lazy"
      draggable="false"
      @error="failed = true"
    >
    <video v-else-if="asset.kind === 'video'" :src="`${asset.url}#t=0.5`" preload="metadata" muted playsinline class="size-full object-cover" />
    <div v-else class="grid size-full place-items-center text-xs text-muted">{{ $t('media.noPreview') }}</div>
    <span v-if="asset.kind === 'video'" class="absolute bottom-1.5 left-1.5 grid size-6 place-items-center rounded-full bg-ink/75 text-[10px] text-white">▶</span>
    <slot />
  </div>
</template>
