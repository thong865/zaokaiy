<script setup lang="ts">
/** Crisp vector barcode (EAN-13 / EAN-8 / UPC-A / Code 128) — prints sharply at any size. */
const props = withDefaults(defineProps<{ value: string; height?: number; showText?: boolean }>(), { height: 50, showText: true })
const pattern = computed(() => barcodePattern(props.value))
const QUIET = 10
const width = computed(() => (pattern.value ? pattern.value.modules.length + QUIET * 2 : 0))
/** Merge consecutive bar modules into rects. */
const bars = computed(() => {
  const out: { x: number; w: number }[] = []
  const m = pattern.value?.modules ?? ''
  for (let i = 0; i < m.length; ) {
    if (m[i] === '1') {
      let j = i
      while (m[j] === '1') j++
      out.push({ x: QUIET + i, w: j - i })
      i = j
    } else i++
  }
  return out
})
const textH = computed(() => (props.showText ? 12 : 0))
</script>

<template>
  <svg
    v-if="pattern"
    :viewBox="`0 0 ${width} ${height + textH}`"
    preserveAspectRatio="none"
    shape-rendering="crispEdges"
    role="img"
    :aria-label="pattern.text"
    class="block"
  >
    <rect :width="width" :height="height + textH" fill="#fff" />
    <rect v-for="(b, i) in bars" :key="i" :x="b.x" y="0" :width="b.w" :height="height" fill="#000" />
    <text v-if="showText" :x="width / 2" :y="height + 10" text-anchor="middle" font-family="ui-monospace, monospace" font-size="10" letter-spacing="1">{{ pattern.text }}</text>
  </svg>
  <span v-else class="text-xs text-brand-700">{{ value || '—' }}</span>
</template>
