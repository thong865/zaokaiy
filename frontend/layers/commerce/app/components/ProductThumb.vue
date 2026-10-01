<script setup lang="ts">
import type { ProductCover } from '~/utils/types'

/** Product/shop image with responsive renditions when available; initial letter as fallback. */
const props = withDefaults(
  defineProps<{ src?: string | null; image?: ProductCover | null; name: string; rounded?: string; sizes?: string; eager?: boolean }>(),
  { src: null, image: null, rounded: undefined, sizes: '(min-width: 1024px) 25vw, 50vw', eager: false },
)
const failed = ref(false)
const url = computed(() => props.image?.url || props.src || '')
watch(url, () => (failed.value = false))
const palette = ['#ffe1d9', '#dff5ea', '#fff1c7', '#e4e8ff', '#f4e1ff', '#dff3ff']
const bg = computed(() => palette[[...props.name].reduce((a, c) => a + c.charCodeAt(0), 0) % palette.length])
</script>

<template>
  <div class="relative overflow-hidden bg-line/40" :class="rounded ?? 'rounded-2xl'">
    <AppImage
      v-if="url && !failed"
      :src="url"
      :alt="image?.alt || name"
      :variants="image?.variants"
      :placeholder="image?.placeholder"
      :color="image?.color"
      :width="image?.width"
      :height="image?.height"
      :sizes="sizes"
      :eager="eager"
      class="size-full"
      @error="failed = true"
    />
    <div v-else class="thumb-ph grid size-full place-items-center" :style="{ '--ph': bg }">
      <span class="text-3xl font-extrabold">{{ name.slice(0, 1).toUpperCase() }}</span>
    </div>
  </div>
</template>
