<script setup lang="ts">
defineProps<{ title: string; text?: string; icon?: string }>()
</script>

<template>
  <UEmpty
    :title="title"
    :description="text"
    :icon="icon ?? 'i-lucide-inbox'"
    variant="naked"
    size="lg"
    class="card py-14"
    :ui="{ avatar: 'size-14 rounded-2xl bg-primary shadow-[0_12px_28px_-10px_var(--ui-primary)] [&>[data-slot=icon]]:text-white', title: 'font-bold text-highlighted', description: 'max-w-sm' }"
  >
    <template v-if="$slots.default" #actions><slot /></template>
  </UEmpty>
</template>
