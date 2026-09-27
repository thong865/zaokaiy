<script setup lang="ts">
const props = defineProps<{ src?: string | null; name: string; rounded?: string }>()
const failed = ref(false)
const palette = ['#ffe1d9', '#dff5ea', '#fff1c7', '#e4e8ff', '#f4e1ff', '#dff3ff']
const bg = computed(() => palette[[...props.name].reduce((a, c) => a + c.charCodeAt(0), 0) % palette.length])
</script>

<template>
  <div class="relative overflow-hidden bg-line/40" :class="rounded ?? 'rounded-2xl'">
    <img v-if="src && !failed" :src="src" :alt="name" class="size-full object-cover" loading="lazy" @error="failed = true" >
    <div v-else class="grid size-full place-items-center" :style="{ background: bg }">
      <span class="text-3xl font-extrabold text-ink/70">{{ name.slice(0, 1).toUpperCase() }}</span>
    </div>
  </div>
</template>
