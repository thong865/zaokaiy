<script setup lang="ts">
const props = defineProps<{ level?: string | null; confirmed?: number; shops?: number; help?: boolean }>()
const lvl = computed(() => props.level || 'none')
</script>

<template>
  <UTooltip :text="$t(`codrisk.levelHelp.${lvl}`)" :disabled="!help">
    <span class="inline-flex flex-col items-start gap-0.5">
      <UBadge :color="riskColor(lvl)" :variant="lvl === 'blocked' ? 'solid' : 'soft'" size="sm" :icon="lvl === 'none' ? 'i-lucide-shield-check' : 'i-lucide-shield-alert'">
        {{ $t(`codrisk.level.${lvl}`) }}
      </UBadge>
      <span v-if="confirmed" class="text-[11px] text-muted">{{ $t('codrisk.counts', { n: confirmed, shops: shops ?? 0 }, confirmed) }}</span>
    </span>
  </UTooltip>
</template>
