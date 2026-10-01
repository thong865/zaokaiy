<script setup lang="ts">
const route = useRoute()
const { t } = useI18n()
const tabs = computed(() => [
  { to: '/dashboard/social', label: t('social.nav.board') },
  { to: '/dashboard/social/orders', label: t('social.nav.orders') },
  { to: '/dashboard/social/channels', label: t('social.nav.channels') },
  { to: '/dashboard/social/settings', label: t('social.nav.settings') },
])
const active = (to: string) => (to === '/dashboard/social' ? route.path === to : route.path.startsWith(to))
</script>

<template>
  <nav class="mb-6 flex gap-1 overflow-x-auto border-b border-line">
    <NuxtLink v-for="tab in tabs" :key="tab.to" :to="tab.to" class="-mb-px whitespace-nowrap border-b-2 px-4 py-2.5 text-sm font-semibold transition" :class="active(tab.to) ? 'border-brand-500 text-ink' : 'border-transparent text-muted hover:text-ink'">
      {{ tab.label }}
    </NuxtLink>
  </nav>
</template>
