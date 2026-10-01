<script setup lang="ts">
import type { MediaAsset } from '~/utils/types'

type TileAsset = Pick<MediaAsset, 'kind' | 'url' | 'thumb_url' | 'alt'> &
  Partial<Pick<MediaAsset, 'variants' | 'placeholder' | 'dominant_color' | 'width' | 'height'>> & { color?: string }
const props = withDefaults(defineProps<{ asset: TileAsset; selected?: boolean; rounded?: string; sizes?: string; showQuality?: boolean }>(), {
  rounded: undefined,
  sizes: '160px',
  showQuality: false,
})
const failed = ref(false)
/** Below this on the shortest side a photo looks soft when zoomed on the product page. */
const RECOMMENDED = 1000
const lowRes = computed(() => props.showQuality && props.asset.kind === 'image' && !!props.asset.width && !!props.asset.height && Math.min(props.asset.width, props.asset.height) < RECOMMENDED)
</script>

<template>
  <div class="relative overflow-hidden bg-line/50" :class="[rounded ?? 'rounded-xl', selected && 'ring-4 ring-brand-500 ring-offset-2']">
    <AppImage
      v-if="asset.kind === 'image' && !failed"
      :src="asset.thumb_url || asset.url"
      :alt="asset.alt"
      :variants="asset.variants"
      :placeholder="asset.placeholder"
      :color="asset.dominant_color || asset.color"
      :sizes="sizes"
      :draggable="false"
      class="size-full"
      @error="failed = true"
    />
    <img v-else-if="asset.thumb_url && !failed" :src="asset.thumb_url" :alt="asset.alt" class="size-full object-cover" loading="lazy" decoding="async" draggable="false" @error="failed = true">
    <video v-else-if="asset.kind === 'video'" :src="`${asset.url}#t=0.5`" preload="metadata" muted playsinline class="size-full object-cover" />
    <div v-else class="grid size-full place-items-center text-xs text-muted">{{ $t('media.noPreview') }}</div>
    <span v-if="asset.kind === 'video'" class="absolute bottom-1.5 left-1.5 grid size-6 place-items-center rounded-full bg-night/75 text-[10px] text-white">▶</span>
    <span
      v-if="lowRes"
      class="absolute left-1.5 top-1.5 rounded-full bg-sun-400 px-1.5 py-0.5 text-[10px] font-bold text-night shadow"
      :title="$t('media.quality.lowResHint', { w: asset.width, h: asset.height, rec: RECOMMENDED })"
    >{{ $t('media.quality.lowRes') }}</span>
    <slot />
  </div>
</template>
