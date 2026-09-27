<script setup lang="ts">
defineProps<{ label: string; value: string | number; hint?: string; accent?: boolean; to?: string }>()
const NuxtLink = resolveComponent('NuxtLink')
</script>

<template>
  <component :is="to ? NuxtLink : 'div'" :to="to" class="card block p-5 transition" :class="[accent && 'border-brand-500/40 bg-brand-50', to && 'hover:border-ink/40']">
    <div class="text-xs font-semibold uppercase tracking-wide text-muted">{{ label }}</div>
    <div class="mt-2 text-2xl font-extrabold tracking-tight">{{ value }}</div>
    <div v-if="hint" class="mt-1 text-xs text-muted">{{ hint }}</div>
  </component>
</template>
