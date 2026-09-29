<script setup lang="ts">
const { user, loggedIn, logout } = useAuth()
const { count } = useCart()
const route = useRoute()
const q = ref((route.query.q as string) || '')
function search() {
  navigateTo({ path: '/', query: q.value ? { q: q.value } : {} })
}
// Header gains a shadow once the page scrolls (Vuesax navbar).
const scrolled = ref(false)
function onScroll() {
  scrolled.value = window.scrollY > 4
}
onMounted(() => {
  onScroll()
  window.addEventListener('scroll', onScroll, { passive: true })
})
onBeforeUnmount(() => window.removeEventListener('scroll', onScroll))
const links = [
  { to: '/', key: 'common.nav.home', icon: 'home', exact: true },
  { to: '/vehicles', key: 'vehicle.nav.vehicles', icon: 'car' },
  { to: '/feed', key: 'common.nav.creators', icon: 'film' },
]
const isActive = (to: string, exact?: boolean) => (exact ? route.path === to : route.path.startsWith(to))
const { t } = useI18n()
const userMenu = computed(() => [
  [{ label: user.value?.display_name ?? '', type: 'label' as const, avatar: { text: initial.value } }],
  [
    { label: t('common.nav.sellerDashboard'), icon: 'i-lucide-layout-dashboard', to: '/dashboard' },
    { label: t('common.nav.myOrders'), icon: 'i-lucide-receipt', to: '/orders' },
    { label: t('common.nav.account'), icon: 'i-lucide-user', to: '/account' },
    ...(user.value?.role === 'admin' ? [{ label: t('common.nav.adminConsole'), icon: 'i-lucide-shield-check', to: '/admin', color: 'primary' as const }] : []),
  ],
  [{ label: t('common.nav.logout'), icon: 'i-lucide-log-out', onSelect: () => logout() }],
])
const initial = computed(() => user.value?.display_name?.slice(0, 1).toUpperCase() ?? '?')
</script>

<template>
  <div class="min-h-dvh pb-20 md:pb-0">
    <header class="sticky top-0 z-30 transition-shadow duration-300" :class="scrolled ? 'glass shadow-[0_5px_20px_0_rgb(0_0_0/0.06)]' : 'bg-paper'">
      <div class="mx-auto flex h-16 max-w-7xl items-center gap-2 px-4 sm:gap-4">
        <AppLogo />
        <nav class="ml-4 hidden items-center gap-1 lg:flex">
          <NuxtLink
            v-for="l in links.slice(1)"
            :key="l.to"
            :to="l.to"
            class="relative rounded-xl px-3 py-2 text-sm font-semibold transition hover:text-brand-500"
            :class="isActive(l.to, l.exact) ? 'text-brand-500' : 'text-ink/75'"
          >
            {{ $t(l.key) }}
            <span v-if="isActive(l.to, l.exact)" class="absolute inset-x-3 -bottom-[13px] h-[3px] rounded-t-full bg-brand-500" />
          </NuxtLink>
        </nav>
        <form class="hidden flex-1 md:block" role="search" @submit.prevent="search">
          <UInput v-model="q" icon="i-lucide-search" class="w-full max-w-md" :placeholder="$t('common.nav.searchPlaceholder')" :aria-label="$t('common.search')" />
        </form>
        <div class="ml-auto flex min-w-0 items-center gap-1">
          <div class="mr-1 hidden sm:block"><LangSwitch /></div>
          <ThemeToggle />
          <ClientOnly>
            <UChip :text="count" :show="count > 0" size="3xl" :ui="{ base: 'font-bold shadow-[0_4px_10px_-2px_var(--ui-primary)]' }" class="hidden md:inline-flex">
              <UButton to="/cart" icon="i-lucide-shopping-bag" color="neutral" variant="ghost" square :aria-label="$t('common.nav.cart')" />
            </UChip>
          </ClientOnly>
          <UDropdownMenu v-if="loggedIn" :items="userMenu" :content="{ align: 'end' }" :ui="{ content: 'w-60' }">
            <UButton color="neutral" variant="ghost" trailing-icon="i-lucide-chevron-down" :aria-label="$t('common.nav.menu')" class="pl-1.5">
              <UAvatar :text="initial" size="sm" class="bg-primary text-white shadow-[0_6px_14px_-6px_var(--ui-primary)]" :ui="{ fallback: 'text-white font-bold' }" />
              <span class="hidden max-w-32 truncate sm:inline">{{ user?.display_name }}</span>
            </UButton>
          </UDropdownMenu>
          <template v-else>
            <UButton color="neutral" variant="soft" size="md" to="/login" class="hidden sm:inline-flex">{{ $t('common.nav.login') }}</UButton>
            <UButton size="md" to="/register">{{ $t('common.nav.openShop') }}</UButton>
          </template>
        </div>
      </div>
    </header>
    <main>
      <slot />
    </main>
    <footer class="mt-24 bg-surface">
      <div class="mx-auto grid max-w-7xl gap-8 px-4 py-12 sm:grid-cols-2 lg:grid-cols-4">
        <div class="space-y-3">
          <AppLogo />
          <p class="max-w-xs text-sm text-muted">{{ $t('common.appTagline') }}</p>
          <div class="flex items-center gap-2"><LangSwitch /><ThemeToggle /></div>
        </div>
        <div>
          <div class="eyebrow">{{ $t('common.footer.shop') }}</div>
          <ul class="mt-3 space-y-2 text-sm">
            <li><NuxtLink to="/" class="text-ink/75 hover:text-brand-500">{{ $t('common.nav.marketplace') }}</NuxtLink></li>
            <li><NuxtLink to="/vehicles" class="text-ink/75 hover:text-brand-500">{{ $t('vehicle.nav.vehicles') }}</NuxtLink></li>
            <li><NuxtLink to="/feed" class="text-ink/75 hover:text-brand-500">{{ $t('common.nav.creators') }}</NuxtLink></li>
          </ul>
        </div>
        <div>
          <div class="eyebrow">{{ $t('common.footer.sell') }}</div>
          <ul class="mt-3 space-y-2 text-sm">
            <li><NuxtLink to="/register" class="text-ink/75 hover:text-brand-500">{{ $t('common.nav.openShop') }}</NuxtLink></li>
            <li><NuxtLink to="/dashboard/marketplace" class="text-ink/75 hover:text-brand-500">{{ $t('store.home.becomeSellStaff') }}</NuxtLink></li>
            <li><NuxtLink to="/dashboard" class="text-ink/75 hover:text-brand-500">{{ $t('common.nav.sellerDashboard') }}</NuxtLink></li>
          </ul>
        </div>
        <div>
          <div class="eyebrow">{{ $t('common.footer.account') }}</div>
          <ul class="mt-3 space-y-2 text-sm">
            <li><NuxtLink to="/orders" class="text-ink/75 hover:text-brand-500">{{ $t('common.nav.myOrders') }}</NuxtLink></li>
            <li><NuxtLink to="/cart" class="text-ink/75 hover:text-brand-500">{{ $t('common.nav.cart') }}</NuxtLink></li>
            <li><NuxtLink to="/account" class="text-ink/75 hover:text-brand-500">{{ $t('common.nav.account') }}</NuxtLink></li>
          </ul>
        </div>
      </div>
      <div class="border-t border-line/70 py-5 text-center text-xs text-muted">© {{ new Date().getFullYear() }} zaokaiy</div>
    </footer>

    <!-- Mobile tab bar -->
    <nav class="glass fixed inset-x-3 bottom-3 z-30 flex items-center justify-around rounded-[20px] p-1.5 shadow-pop md:hidden" :aria-label="$t('common.nav.menu')">
      <NuxtLink v-for="l in links" :key="l.to" :to="l.to" class="flex flex-1 flex-col items-center gap-0.5 rounded-2xl py-1.5 text-[10px] font-semibold transition" :class="isActive(l.to, l.exact) ? 'text-brand-500' : 'text-muted'">
        <AppIcon :name="l.icon" class="size-5" />{{ $t(l.key) }}
      </NuxtLink>
      <NuxtLink to="/cart" class="relative flex flex-1 flex-col items-center gap-0.5 rounded-2xl py-1.5 text-[10px] font-semibold transition" :class="isActive('/cart') ? 'text-brand-500' : 'text-muted'">
        <AppIcon name="bag" class="size-5" />{{ $t('common.nav.cart') }}
        <ClientOnly><span v-if="count" class="absolute right-[22%] top-0.5 grid min-w-4 place-items-center rounded-full bg-brand-500 px-1 text-[9px] font-bold leading-4 text-white">{{ count }}</span></ClientOnly>
      </NuxtLink>
      <NuxtLink :to="loggedIn ? '/dashboard' : '/login'" class="flex flex-1 flex-col items-center gap-0.5 rounded-2xl py-1.5 text-[10px] font-semibold text-muted transition">
        <AppIcon name="user" class="size-5" />{{ loggedIn ? $t('common.nav.dashboard') : $t('common.nav.login') }}
      </NuxtLink>
    </nav>
  </div>
</template>
