<script setup lang="ts">
withDefaults(defineProps<{ dark?: boolean }>(), { dark: false })
</script>

<template>
  <UTooltip :text="$t('kyb.badge.tooltip')">
    <UBadge icon="i-lucide-badge-check" color="success" :variant="dark ? 'solid' : 'soft'" size="sm" :class="dark && 'bg-white/15 text-white'">{{ $t('kyb.badge.verified') }}</UBadge>
  </UTooltip>
</template>
