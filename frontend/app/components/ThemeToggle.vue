<script setup lang="ts">
/** Cycles light → dark → system (stored in the zk_theme cookie by @nuxtjs/color-mode). `dark` for dark bars. */
withDefaults(defineProps<{ dark?: boolean }>(), { dark: false })
const mode = useColorMode()
const order = ['light', 'dark', 'system'] as const
const current = computed(() => (order.includes(mode.preference as never) ? (mode.preference as (typeof order)[number]) : 'system'))
const icon = computed(() => ({ light: 'i-lucide-sun', dark: 'i-lucide-moon', system: 'i-lucide-monitor' })[current.value])
function cycle() {
  mode.preference = order[(order.indexOf(current.value) + 1) % order.length]!
}
</script>

<template>
  <ClientOnly>
    <UTooltip :text="$t(`common.theme.${current}`)">
      <UButton
        :icon="icon"
        color="neutral"
        variant="ghost"
        square
        :class="dark && 'text-white/75 hover:bg-white/10 hover:text-white'"
        :aria-label="$t('common.theme.label', { mode: $t(`common.theme.${current}`) })"
        @click="cycle"
      />
    </UTooltip>
    <template #fallback><span class="inline-block size-9" /></template>
  </ClientOnly>
</template>
