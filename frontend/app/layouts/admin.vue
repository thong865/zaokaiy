<script setup lang="ts">
const { user, logout } = useAuth()
const route = useRoute()
const api = useApi()
const { data: overview } = useLazyAsyncData('admin-overview-nav', () => api<{ pending: number }>('/admin/overview'), {
  server: false,
  watch: [() => route.fullPath],
})
const { t } = useI18n()
const nav = computed(() => [
  { to: '/admin', label: t('common.adminNav.review'), icon: '✓' },
  { to: '/admin/categories', label: t('common.adminNav.categories'), icon: '☰' },
  { to: '/admin/shops', label: t('common.adminNav.shops'), icon: '▦' },
])
const isActive = (to: string) => (to === '/admin' ? route.path === '/admin' || route.path.startsWith('/admin/products') : route.path.startsWith(to))
</script>

<template>
  <div class="min-h-dvh">
    <header class="sticky top-0 z-30 border-b border-white/10 bg-ink text-white">
      <div class="mx-auto flex h-14 max-w-7xl items-center gap-6 px-4">
        <NuxtLink to="/admin" class="flex items-center gap-2 font-extrabold">
          <span class="grid size-7 place-items-center rounded-lg bg-brand-500 text-sm">Z</span>
          zaokaiy <span class="rounded bg-white/10 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider">{{ $t('common.nav.admin') }}</span>
        </NuxtLink>
        <nav class="flex gap-1 overflow-x-auto text-sm font-semibold">
          <NuxtLink v-for="n in nav" :key="n.to" :to="n.to" class="flex items-center gap-2 whitespace-nowrap rounded-full px-3 py-1.5 transition" :class="isActive(n.to) ? 'bg-white text-ink' : 'text-white/70 hover:text-white'">
            {{ n.label }}
            <span v-if="n.to === '/admin' && overview?.pending" class="rounded-full bg-brand-500 px-1.5 text-[10px] text-white">{{ overview.pending }}</span>
          </NuxtLink>
        </nav>
        <div class="ml-auto flex items-center gap-3 text-sm">
          <LangSwitch dark />
          <NuxtLink to="/" class="hidden text-white/70 hover:text-white sm:inline">{{ $t('common.nav.marketplace') }} ↗</NuxtLink>
          <span class="hidden text-white/50 md:inline">{{ user?.email }}</span>
          <button class="text-white/70 hover:text-white" @click="logout">{{ $t('common.nav.logout') }}</button>
        </div>
      </div>
    </header>
    <main class="mx-auto max-w-7xl px-4 py-8">
      <slot />
    </main>
  </div>
</template>
