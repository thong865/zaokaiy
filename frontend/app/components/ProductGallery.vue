<script setup lang="ts">
import type { PublicMedia } from '~/utils/types'

const props = defineProps<{ media: PublicMedia[]; name: string }>()
const i = ref(0)
const current = computed(() => props.media[i.value])
function go(d: number) {
  if (!props.media.length) return
  i.value = (i.value + d + props.media.length) % props.media.length
}
let startX = 0
const onTouchStart = (e: TouchEvent) => (startX = e.touches[0]?.clientX ?? 0)
function onTouchEnd(e: TouchEvent) {
  const dx = (e.changedTouches[0]?.clientX ?? 0) - startX
  if (Math.abs(dx) > 40) go(dx < 0 ? 1 : -1)
}
</script>

<template>
  <div>
    <div
      class="group relative aspect-square overflow-hidden rounded-3xl bg-line/40 outline-none"
      tabindex="0"
      @keydown.left="go(-1)"
      @keydown.right="go(1)"
      @touchstart.passive="onTouchStart"
      @touchend="onTouchEnd"
    >
      <template v-if="current">
        <video
          v-if="current.kind === 'video'"
          :key="current.id"
          :src="current.url"
          :poster="current.thumb_url ?? undefined"
          controls
          playsinline
          class="size-full bg-ink object-contain"
        />
        <img v-else :key="current.id" :src="current.url" :alt="current.alt || name" class="size-full object-cover" >
      </template>
      <ProductThumb v-else :name="name" class="size-full" rounded="rounded-none" />
      <template v-if="media.length > 1">
        <button class="absolute left-3 top-1/2 grid size-10 -translate-y-1/2 place-items-center rounded-full bg-white/90 opacity-0 shadow transition group-hover:opacity-100" :aria-label="$t('common.previous')" @click="go(-1)">‹</button>
        <button class="absolute right-3 top-1/2 grid size-10 -translate-y-1/2 place-items-center rounded-full bg-white/90 opacity-0 shadow transition group-hover:opacity-100" :aria-label="$t('common.next')" @click="go(1)">›</button>
        <span class="absolute bottom-3 right-3 rounded-full bg-ink/70 px-2.5 py-0.5 text-xs font-semibold text-white">{{ i + 1 }} / {{ media.length }}</span>
      </template>
    </div>
    <div v-if="media.length > 1" class="mt-3 flex gap-2 overflow-x-auto pb-1">
      <button
        v-for="(m, j) in media"
        :key="m.id"
        class="size-16 shrink-0 overflow-hidden rounded-xl border-2 transition"
        :class="j === i ? 'border-ink' : 'border-transparent opacity-70 hover:opacity-100'"
        :aria-label="$t(m.kind === 'video' ? 'store.gallery.video' : 'store.gallery.image', { n: j + 1 })"
        @click="i = j"
      >
        <MediaTile :asset="m" class="size-full" rounded="rounded-none" />
      </button>
    </div>
  </div>
</template>
