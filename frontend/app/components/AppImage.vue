<script setup lang="ts">
import type { MediaVariant } from '~/utils/types'

/**
 * Fast, responsive image.
 * - `<picture>` with a WebP `srcset` (server renditions) so each device downloads the smallest
 *   file that is still sharp for its layout width (`sizes`) and pixel density; JPEG/PNG fallback.
 * - Native lazy loading + async decoding; `eager` for the one hero image above the fold
 *   (also sets fetchpriority=high for a better LCP).
 * - Tiny blurred placeholder + dominant colour while loading, and width/height so the layout
 *   never jumps (no CLS).
 */
const props = withDefaults(
  defineProps<{
    src: string
    alt?: string
    variants?: MediaVariant[] | null
    placeholder?: string | null
    color?: string | null
    width?: number | null
    height?: number | null
    /** Layout width hint for the browser, e.g. "(min-width: 1024px) 25vw, 50vw". */
    sizes?: string
    eager?: boolean
    fit?: 'cover' | 'contain'
    draggable?: boolean
  }>(),
  { alt: '', variants: null, placeholder: null, color: null, width: null, height: null, sizes: '100vw', eager: false, fit: 'cover', draggable: true },
)
const emit = defineEmits<{ error: [] }>()

const srcset = computed(() =>
  (props.variants ?? [])
    .filter((v) => v?.url && v.w > 0)
    .map((v) => `${v.url} ${v.w}w`)
    .join(', '),
)
const hasPlaceholder = computed(() => !!props.placeholder && props.placeholder.startsWith('data:'))
const loaded = ref(false)
const img = ref<HTMLImageElement | null>(null)
// SSR-rendered images may finish loading before hydration (no load event afterwards).
onMounted(() => {
  if (img.value?.complete && img.value.naturalWidth > 0) loaded.value = true
})
watch(() => props.src, () => (loaded.value = false))
</script>

<template>
  <div class="relative overflow-hidden" :style="color ? { backgroundColor: color } : undefined">
    <img
      v-if="hasPlaceholder && !loaded"
      :src="placeholder!"
      alt=""
      aria-hidden="true"
      class="absolute inset-0 size-full scale-110 object-cover blur-xl"
      :class="fit === 'contain' && 'object-contain'"
    >
    <picture>
      <source v-if="srcset" type="image/webp" :srcset="srcset" :sizes="sizes">
      <img
        ref="img"
        :src="src"
        :alt="alt"
        :width="width ?? undefined"
        :height="height ?? undefined"
        :loading="eager ? 'eager' : 'lazy'"
        :fetchpriority="eager ? 'high' : 'auto'"
        decoding="async"
        :draggable="draggable"
        class="relative size-full transition-opacity duration-500"
        :class="[fit === 'contain' ? 'object-contain' : 'object-cover', loaded || !hasPlaceholder ? 'opacity-100' : 'opacity-0']"
        @load="loaded = true"
        @error="emit('error')"
      >
    </picture>
  </div>
</template>
