<script setup lang="ts">
defineProps<{ label: string; value: string | number; hint?: string; accent?: boolean; to?: string }>()
const NuxtLink = resolveComponent('NuxtLink')
</script>

<template>
  <component :is="to ? NuxtLink : 'div'" :to="to" class="card relative block overflow-hidden p-5" :class="[accent && 'bg-brand-500 text-white shadow-[0_14px_30px_-14px_var(--brand-500)]', to && 'card-hover']">
    <div class="text-xs font-semibold" :class="accent ? 'text-white/80' : 'text-muted'">{{ label }}</div>
    <div class="mt-2 text-2xl font-extrabold tracking-tight">{{ value }}</div>
    <div v-if="hint" class="mt-1 text-xs" :class="accent ? 'text-white/75' : 'text-muted'">{{ hint }}</div>
    <div v-if="accent" class="pointer-events-none absolute -right-8 -top-8 size-28 rounded-full bg-white/15" />
  </component>
</template>
