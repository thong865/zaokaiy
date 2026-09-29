<script setup lang="ts">
import type { KybStatus } from '~/utils/types'

defineProps<{ status: KybStatus; verified?: boolean }>()
const color: Record<KybStatus, 'neutral' | 'info' | 'warning' | 'success' | 'error'> = {
  none: 'neutral',
  draft: 'neutral',
  submitted: 'info',
  changes_requested: 'warning',
  approved: 'success',
  rejected: 'error',
  revoked: 'neutral',
}
</script>

<template>
  <UBadge :color="color[status]" :variant="status === 'approved' || status === 'rejected' || status === 'revoked' ? 'solid' : 'soft'" size="sm">
    {{ $t(`kyb.status.${status}`) }}<template v-if="verified && status !== 'approved'"> · {{ $t('kyb.badge.verified') }}</template>
  </UBadge>
</template>
