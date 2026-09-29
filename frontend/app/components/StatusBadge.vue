<script setup lang="ts">
const props = defineProps<{ status: string; label?: string }>()
type Color = 'success' | 'info' | 'secondary' | 'warning' | 'neutral' | 'error'
const tone: Record<string, Color> = {
  active: 'success',
  approved: 'success',
  completed: 'success',
  published: 'success',
  paid: 'info',
  shipped: 'secondary',
  pending: 'warning',
  draft: 'neutral',
  paused: 'neutral',
  archived: 'neutral',
  ended: 'neutral',
  rejected: 'error',
  revoked: 'error',
  cancelled: 'error',
  void: 'error',
  not_submitted: 'neutral',
  open: 'warning',
  confirmed: 'info',
  expired: 'neutral',
  claimed: 'success',
  partial: 'warning',
  sold_out: 'error',
  ignored: 'neutral',
  voided: 'error',
  collected: 'info',
  remitted: 'success',
  returned: 'error',
}
const color = computed<Color>(() => tone[props.status] ?? 'neutral')
</script>

<template>
  <UBadge :color="color" variant="soft" size="sm" class="capitalize">{{ label ?? ($te(`common.status.${status}`) ? $t(`common.status.${status}`) : status.replace('_', ' ')) }}</UBadge>
</template>
