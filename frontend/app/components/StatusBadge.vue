<script setup lang="ts">
const props = defineProps<{ status: string; label?: string }>()
const tone: Record<string, string> = {
  active: 'bg-mint-500/15 text-mint-500',
  approved: 'bg-mint-500/15 text-mint-500',
  completed: 'bg-mint-500/15 text-mint-500',
  published: 'bg-mint-500/15 text-mint-500',
  paid: 'bg-blue-500/10 text-blue-600',
  shipped: 'bg-indigo-500/10 text-indigo-600',
  pending: 'bg-sun-400/25 text-amber-700',
  draft: 'bg-line text-muted',
  paused: 'bg-line text-muted',
  archived: 'bg-line text-muted',
  ended: 'bg-line text-muted',
  rejected: 'bg-brand-50 text-brand-700',
  revoked: 'bg-brand-50 text-brand-700',
  cancelled: 'bg-brand-50 text-brand-700',
  void: 'bg-brand-50 text-brand-700',
  not_submitted: 'bg-line text-muted',
  open: 'bg-sun-400/25 text-amber-700',
  confirmed: 'bg-blue-500/10 text-blue-600',
  expired: 'bg-line text-muted',
  claimed: 'bg-mint-500/15 text-mint-500',
  partial: 'bg-sun-400/25 text-amber-700',
  sold_out: 'bg-brand-50 text-brand-700',
  ignored: 'bg-line text-muted',
  voided: 'bg-brand-50 text-brand-700',
}
const cls = computed(() => tone[props.status] ?? 'bg-line text-muted')
</script>

<template>
  <span class="chip capitalize" :class="cls">{{ label ?? ($te(`common.status.${status}`) ? $t(`common.status.${status}`) : status.replace('_', ' ')) }}</span>
</template>
