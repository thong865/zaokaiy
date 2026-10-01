<script setup lang="ts">
const { shops, shop, select, can } = useShop()
const { user, logout } = useAuth()
const route = useRoute()
const open = ref(false)
watch(() => route.fullPath, () => (open.value = false))

const { t } = useI18n()
const { withModules } = useModules()
// Navigation is contributed by the cores (layers/*/app/plugins/core.ts), filtered by shop type,
// extended by the enabled plug-in modules and — for staff — by what their role allows.
const { nav } = useCores()
const groups = computed(() => {
  const base = nav(shop.value?.vertical, shop.value?.entity_type === 'business')
  // Plug-in modules (COD risk, POS terminals …) extend the product-selling shop types only.
  const sellsProducts = base.some((g) => g.items.some((i) => i.to === '/dashboard/products'))
  return base
    .map((g) => {
      const items = g.items.map((i) => ({ to: i.to, label: t(i.label), icon: i.icon }))
      return { label: t(g.label), items: sellsProducts ? withModules('dashboardNav', items, g.id) : items }
    })
    .map((g) => ({ ...g, items: g.items.filter((it) => can(pagePermission(it.to))) }))
    .filter((g) => g.items.length)
})
const isActive = (to: string) => (to === '/dashboard' ? route.path === to : route.path === to || route.path.startsWith(`${to}/`))
// Nuxt UI navigation: one list per group, headed by a label item.
const menu = computed(() =>
  groups.value.map((g) => [
    { label: g.label, type: 'label' as const },
    ...g.items.map((it) => ({ label: it.label, icon: `i-lucide-${ICON[it.icon] ?? it.icon}`, to: it.to, active: isActive(it.to) })),
  ]),
)
const ICON: Record<string, string> = {
  dashboard: 'layout-dashboard', folder: 'folder-tree', file: 'file-text', barcode: 'scan-barcode', message: 'message-circle',
  pen: 'pen-line', settings: 'sliders-horizontal', shield: 'shield-check', receipt: 'receipt',
}
const shopItems = computed(() => [
  shops.value.map((s) => ({ label: s.access && !s.access.owner ? `${s.name} · ${s.access.role}` : s.name, avatar: { text: s.name.slice(0, 1).toUpperCase() }, onSelect: () => select(s.id), active: s.id === shop.value?.id })),
  [{ label: t('common.nav.viewStorefront').replace(' ↗', ''), icon: 'i-lucide-external-link', to: shop.value ? `/s/${shop.value.slug}` : '/', target: '_blank' }],
])
const userItems = computed(() => [
  [
    { label: t('common.nav.marketplace'), icon: 'i-lucide-store', to: '/' },
    { label: t('common.nav.account'), icon: 'i-lucide-user', to: '/account' },
    ...(user.value?.role === 'admin' ? [{ label: t('common.nav.admin'), icon: 'i-lucide-shield-check', to: '/admin' }] : []),
  ],
  [{ label: t('common.nav.logout'), icon: 'i-lucide-log-out', onSelect: () => logout() }],
])
const title = computed(() => groups.value.flatMap((g) => g.items).find((i) => isActive(i.to))?.label ?? t('common.nav.dashboard'))
</script>

<template>
  <UDashboardGroup unit="rem" storage="cookie" storage-key="zk_sidebar">
    <UDashboardSidebar
      id="dashboard"
      v-model:open="open"
      collapsible
      resizable
      :min-size="15"
      :default-size="17"
      :collapsed-size="4.5"
      :ui="{ header: 'px-4', body: 'px-3 gap-3', footer: 'px-3 pb-3 block' }"
    >
      <template #header="{ collapsed }">
        <AppLogo v-if="!collapsed" />
        <NuxtLink v-else to="/" class="mx-auto" aria-label="zaokaiy">
          <svg viewBox="0 0 64 64" class="size-8 drop-shadow-[0_6px_10px_rgb(255_77_46/0.35)]"><rect width="64" height="64" rx="18" fill="#ff4d2e" /><path d="M19 20h26L21 44h24" fill="none" stroke="#fff" stroke-width="7" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </NuxtLink>
      </template>

      <template #default="{ collapsed }">
        <UDropdownMenu v-if="shops.length" :items="shopItems" :content="{ align: 'start' }" :ui="{ content: 'w-(--reka-dropdown-menu-trigger-width) min-w-56' }">
          <UButton color="neutral" variant="soft" block :square="collapsed" :trailing-icon="collapsed ? undefined : 'i-lucide-chevrons-up-down'" class="justify-start rounded-2xl p-2" :aria-label="$t('common.shop')">
            <UAvatar :text="shop?.name?.slice(0, 1).toUpperCase()" size="sm" class="bg-primary shadow-[0_6px_14px_-6px_var(--ui-primary)]" :ui="{ fallback: 'text-white font-bold' }" />
            <span v-if="!collapsed" class="min-w-0 flex-1 truncate text-left">{{ shop?.name }}</span>
          </UButton>
        </UDropdownMenu>
        <UNavigationMenu
          :items="menu"
          orientation="vertical"
          :collapsed="collapsed"
          tooltip
          popover
          :ui="{ label: 'px-3 pt-3 pb-1 text-[10px] font-bold uppercase tracking-[0.14em] text-muted', link: 'px-3 py-2' }"
        />
      </template>

      <template #footer="{ collapsed }">
        <div class="rounded-2xl bg-[var(--field)] p-2">
          <UDropdownMenu :items="userItems" :content="{ align: 'start', side: 'top' }" :ui="{ content: 'w-56' }">
            <UButton color="neutral" variant="ghost" block :square="collapsed" class="justify-start p-1.5" :aria-label="$t('common.nav.account')">
              <UAvatar :text="user?.display_name?.slice(0, 1).toUpperCase()" size="sm" class="bg-inverted" :ui="{ fallback: 'text-inverted font-bold' }" />
              <span v-if="!collapsed" class="min-w-0 flex-1 truncate text-left">{{ user?.display_name }}</span>
            </UButton>
          </UDropdownMenu>
          <div class="mt-1 flex items-center gap-1" :class="collapsed ? 'flex-col' : 'justify-between'">
            <LangSwitch v-if="!collapsed" />
            <div class="flex items-center" :class="collapsed && 'flex-col'">
              <ThemeToggle />
              <UDashboardSidebarCollapse color="neutral" variant="ghost" />
            </div>
          </div>
        </div>
      </template>
    </UDashboardSidebar>

    <UDashboardPanel id="dashboard-main" :ui="{ body: 'p-0 sm:p-0' }">
      <template #header>
        <UDashboardNavbar class="glass sticky top-0 z-10 border-b-0 shadow-[0_5px_20px_0_rgb(0_0_0/0.04)]">
          <template #left>
            <span class="truncate text-base font-semibold text-highlighted">{{ title }}</span>
          </template>
          <template #right>
            <UButton v-if="shop" :to="`/s/${shop.slug}`" target="_blank" icon="i-lucide-external-link" color="neutral" variant="soft" size="md" class="hidden sm:inline-flex">{{ $t('common.nav.viewStorefront').replace(' ↗', '') }}</UButton>
            <ThemeToggle />
          </template>
        </UDashboardNavbar>
      </template>
      <template #body>
        <main class="mx-auto w-full px-2 py-6 lg:px-8 lg:py-8">
          <slot />
        </main>
      </template>
    </UDashboardPanel>
  </UDashboardGroup>
</template>
