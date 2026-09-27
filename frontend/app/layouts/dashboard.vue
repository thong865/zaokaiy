<script setup lang="ts">
const { shops, shop, select } = useShop()
const { user, logout } = useAuth()
const route = useRoute()
const open = ref(false)
watch(() => route.fullPath, () => (open.value = false))

const { t } = useI18n()
const groups = computed(() => [
  {
    label: t('common.dashNav.shop'),
    items: [
      { to: '/dashboard', label: t('common.dashNav.overview'), icon: '◎' },
      { to: '/dashboard/products', label: t('common.dashNav.products'), icon: '▦' },
      { to: '/dashboard/categories', label: t('common.dashNav.categories'), icon: '☰' },
      { to: '/dashboard/inventory', label: t('common.dashNav.inventory'), icon: '▤' },
      { to: '/dashboard/media', label: t('common.dashNav.media'), icon: '▣' },
      { to: '/dashboard/orders', label: t('common.dashNav.orders'), icon: '⇄' },
      { to: '/pos', label: t('common.dashNav.pos'), icon: '⌧' },
      { to: '/dashboard/pos-sales', label: t('common.dashNav.posSales'), icon: '🧾' },
    ],
  },
  {
    label: t('common.dashNav.social'),
    items: [
      { to: '/dashboard/social', label: t('common.dashNav.socialLive'), icon: '💬' },
    ],
  },
  {
    label: t('common.dashNav.network'),
    items: [
      { to: '/dashboard/partners', label: t('common.dashNav.partners'), icon: '⚇' },
      { to: '/dashboard/marketplace', label: t('common.dashNav.marketplace'), icon: '⊕' },
      { to: '/dashboard/commissions', label: t('common.dashNav.commissions'), icon: '%' },
    ],
  },
  {
    label: t('common.dashNav.grow'),
    items: [
      { to: '/dashboard/ads', label: t('common.dashNav.ads'), icon: '◆' },
      { to: '/dashboard/content', label: t('common.dashNav.content'), icon: '✎' },
      { to: '/dashboard/assistant', label: t('common.dashNav.assistant'), icon: '✦' },
      { to: '/dashboard/settings', label: t('common.dashNav.settings'), icon: '⚙' },
    ],
  },
])
const isActive = (to: string) => (to === '/dashboard' ? route.path === to : route.path.startsWith(to))
</script>

<template>
  <div class="min-h-dvh lg:grid lg:grid-cols-[260px_1fr]">
    <aside
      class="fixed inset-y-0 left-0 z-40 w-[260px] -translate-x-full border-r border-line bg-white transition lg:sticky lg:top-0 lg:h-dvh lg:translate-x-0"
      :class="open && 'translate-x-0'"
    >
      <div class="flex h-full flex-col">
        <div class="p-5"><AppLogo /></div>
        <div v-if="shops.length" class="px-4">
          <label class="label">{{ $t('common.shop') }}</label>
          <select class="input" :value="shop?.id" @change="select(($event.target as HTMLSelectElement).value)">
            <option v-for="s in shops" :key="s.id" :value="s.id">{{ s.name }}</option>
          </select>
          <NuxtLink v-if="shop" :to="`/s/${shop.slug}`" target="_blank" class="mt-2 block text-xs font-semibold text-brand-600 hover:underline">
            {{ $t('common.nav.viewStorefront') }}
          </NuxtLink>
        </div>
        <nav class="mt-4 flex-1 overflow-y-auto px-3 pb-4">
          <div v-for="g in groups" :key="g.label" class="mt-3">
            <div class="px-3 pb-1 text-[10px] font-bold uppercase tracking-widest text-muted">{{ g.label }}</div>
            <NuxtLink
              v-for="it in g.items"
              :key="it.to"
              :to="it.to"
              class="flex items-center gap-3 rounded-xl px-3 py-2 text-sm font-semibold transition"
              :class="isActive(it.to) ? 'bg-ink text-white' : 'text-ink/80 hover:bg-paper'"
            >
              <span class="w-4 text-center opacity-80">{{ it.icon }}</span>{{ it.label }}
            </NuxtLink>
          </div>
        </nav>
        <div class="border-t border-line p-4 text-sm">
          <div class="flex items-center justify-between gap-2"><span class="truncate font-semibold">{{ user?.display_name }}</span><LangSwitch /></div>
          <div class="flex gap-3 text-xs text-muted">
            <NuxtLink to="/" class="hover:text-ink">{{ $t('common.nav.marketplace') }}</NuxtLink>
            <NuxtLink v-if="user?.role === 'admin'" to="/admin" class="font-semibold text-brand-600 hover:text-ink">{{ $t('common.nav.admin') }}</NuxtLink>
            <button class="hover:text-ink" @click="logout">{{ $t('common.nav.logout') }}</button>
          </div>
        </div>
      </div>
    </aside>
    <div v-if="open" class="fixed inset-0 z-30 bg-ink/30 lg:hidden" @click="open = false" />
    <div class="min-w-0">
      <div class="sticky top-0 z-20 flex h-14 items-center gap-3 border-b border-line bg-paper/85 px-4 backdrop-blur lg:hidden">
        <button class="btn-ghost btn-sm" @click="open = true">☰</button>
        <span class="font-bold">{{ shop?.name ?? $t('common.nav.dashboard') }}</span>
        <LangSwitch class="ml-auto" />
      </div>
      <main class="mx-auto max-w-6xl px-4 py-8 lg:px-8">
        <slot />
      </main>
    </div>
  </div>
</template>
