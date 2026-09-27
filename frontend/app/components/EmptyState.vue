<script setup lang="ts">
defineProps<{ title: string; text?: string }>()
</script>

<template>
  <div class="card grid place-items-center px-6 py-14 text-center">
    <div class="mb-3 grid size-12 place-items-center rounded-2xl bg-brand-50 text-xl">✦</div>
    <div class="font-bold">{{ title }}</div>
    <p v-if="text" class="mt-1 max-w-sm text-sm text-muted">{{ text }}</p>
    <div class="mt-4"><slot /></div>
  </div>
</template>
