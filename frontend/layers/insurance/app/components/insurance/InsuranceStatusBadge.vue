<script setup lang="ts">
const props = defineProps<{ status: string }>()
const color = computed(() =>
  ({ submitted: 'info', reviewing: 'warning', approved: 'secondary', issued: 'success', rejected: 'error', cancelled: 'neutral' } as const)[props.status as 'submitted'] ?? 'neutral',
)
</script>

<template>
  <UBadge :color="color" :label="$t(`insurance.status.${status}`)" />
</template>
