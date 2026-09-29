<script setup lang="ts">
const { user, logout } = useAuth()
const route = useRoute()
const api = useApi()
const { data: overview } = useLazyAsyncData('admin-overview-nav', () => api<{ pending: number; kyb_pending: number }>('/admin/overview'), {
  server: false,
  watch: [() => route.fullPath],
})
const { t } = useI18n()
const nav = computed(() => [
  { to: '/admin', label: t('common.adminNav.review'), icon: 'check-circle' },
  { to: '/admin/categories', label: t('common.adminNav.categories'), icon: 'folder' },
  { to: '/admin/shops', label: t('common.adminNav.shops'), icon: 'store' },
  { to: '/admin/kyb', label: t('kyb.admin.nav'), icon: 'shield' },
  { to: '/admin/carriers', label: t('common.adminNav.carriers'), icon: 'truck' },
])
const isActive = (to: string) => (to === '/admin' ? route.path === '/admin' || route.path.startsWith('/admin/products') : route.path.startsWith(to))
</script>

<template>
  <div class="min-h-dvh">
    <header class="sticky top-0 z-30 bg-night text-white shadow-[0_5px_20px_0_rgb(0_0_0/0.25)]">
      <div class="mx-auto flex h-16 max-w-7xl items-center gap-5 px-4">
        <NuxtLink to="/admin" class="flex shrink-0 items-center gap-2 font-extrabold">
          <svg viewBox="0 0 64 64" class="size-8"><rect width="64" height="64" rx="18" fill="#ff4d2e" /><path d="M19 20h26L21 44h24" fill="none" stroke="#fff" stroke-width="7" stroke-linecap="round" stroke-linejoin="round" /></svg>
          <span class="hidden sm:inline">zaokaiy</span>
          <span class="rounded-md bg-white/10 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider">{{ $t('common.nav.admin') }}</span>
        </NuxtLink>
        <nav class="flex gap-1 overflow-x-auto text-sm font-semibold">
          <UButton
            v-for="n in nav"
            :key="n.to"
            :to="n.to"
            :icon="n.icon === 'check-circle' ? 'i-lucide-circle-check' : n.icon === 'folder' ? 'i-lucide-folder-tree' : n.icon === 'shield' ? 'i-lucide-shield-check' : `i-lucide-${n.icon}`"
            size="md"
            :active="isActive(n.to)"
            color="neutral"
            variant="ghost"
            active-color="primary"
            active-variant="solid"
            class="whitespace-nowrap"
            :class="!isActive(n.to) && 'text-white/65 hover:bg-white/10 hover:text-white'"
          >
            {{ n.label }}
            <UBadge v-if="n.to === '/admin' && overview?.pending" :label="String(overview.pending)" size="sm" color="neutral" variant="solid" class="bg-white/20 text-white" />
            <UBadge v-if="n.to === '/admin/kyb' && overview?.kyb_pending" :label="String(overview.kyb_pending)" size="sm" color="neutral" variant="solid" class="bg-white/20 text-white" />
          </UButton>
        </nav>
        <div class="ml-auto flex shrink-0 items-center gap-1 text-sm">
          <LangSwitch dark />
          <ThemeToggle dark />
          <UTooltip :text="$t('common.nav.marketplace')"><UButton icon="i-lucide-store" color="neutral" variant="ghost" square to="/" class="text-white/70 hover:bg-white/10 hover:text-white" :aria-label="$t('common.nav.marketplace')" /></UTooltip>
          <UTooltip :text="$t('common.nav.logout')"><UButton icon="i-lucide-log-out" color="neutral" variant="ghost" square class="text-white/70 hover:bg-white/10 hover:text-white" :aria-label="$t('common.nav.logout')" @click="logout" /></UTooltip>
        </div>
      </div>
    </header>
    <main class="mx-auto max-w-7xl px-4 py-8">
      <slot />
    </main>
  </div>
</template>
