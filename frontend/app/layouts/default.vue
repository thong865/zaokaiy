<script setup lang="ts">
const { user, loggedIn, logout } = useAuth()
const { count } = useCart()
const route = useRoute()
const q = ref((route.query.q as string) || '')
const menu = ref(false)
function search() {
  navigateTo({ path: '/', query: q.value ? { q: q.value } : {} })
}
watch(() => route.fullPath, () => (menu.value = false))
</script>

<template>
  <div class="min-h-dvh">
    <header class="sticky top-0 z-30 border-b border-line bg-paper/85 backdrop-blur">
      <div class="mx-auto flex h-16 max-w-7xl items-center gap-4 px-4">
        <AppLogo />
        <form class="hidden flex-1 md:block" @submit.prevent="search">
          <input v-model="q" class="input max-w-md rounded-full" :placeholder="$t('common.nav.searchPlaceholder')" >
        </form>
        <nav class="ml-auto flex items-center gap-1 text-sm font-semibold">
          <LangSwitch class="mr-1" />
          <NuxtLink to="/feed" class="hidden rounded-full px-3 py-2 hover:bg-white sm:block">{{ $t('common.nav.creators') }}</NuxtLink>
          <NuxtLink to="/cart" class="relative rounded-full px-3 py-2 hover:bg-white">
            {{ $t('common.nav.cart') }}
            <ClientOnly>
              <span v-if="count" class="absolute -right-0.5 -top-0.5 grid size-5 place-items-center rounded-full bg-brand-500 text-[10px] text-white">{{ count }}</span>
            </ClientOnly>
          </NuxtLink>
          <template v-if="loggedIn">
            <div class="relative">
              <button class="flex items-center gap-2 rounded-full py-1 pl-1 pr-3 hover:bg-white" @click="menu = !menu">
                <span class="grid size-8 place-items-center rounded-full bg-ink text-xs text-white">{{ user?.display_name?.slice(0, 1).toUpperCase() }}</span>
                <span class="hidden sm:inline">{{ user?.display_name }}</span>
              </button>
              <div v-if="menu" class="card absolute right-0 mt-2 w-48 overflow-hidden p-1 shadow-xl">
                <NuxtLink to="/dashboard" class="block rounded-lg px-3 py-2 hover:bg-paper">{{ $t('common.nav.sellerDashboard') }}</NuxtLink>
                <NuxtLink v-if="user?.role === 'admin'" to="/admin" class="block rounded-lg px-3 py-2 font-bold text-brand-600 hover:bg-paper">{{ $t('common.nav.adminConsole') }}</NuxtLink>
                <NuxtLink to="/orders" class="block rounded-lg px-3 py-2 hover:bg-paper">{{ $t('common.nav.myOrders') }}</NuxtLink>
                <button class="block w-full rounded-lg px-3 py-2 text-left hover:bg-paper" @click="logout">{{ $t('common.nav.logout') }}</button>
              </div>
            </div>
          </template>
          <template v-else>
            <NuxtLink to="/login" class="whitespace-nowrap rounded-full px-3 py-2 hover:bg-white">{{ $t('common.nav.login') }}</NuxtLink>
            <NuxtLink to="/register" class="btn-dark btn-sm whitespace-nowrap">{{ $t('common.nav.openShop') }}</NuxtLink>
          </template>
        </nav>
      </div>
    </header>
    <main>
      <slot />
    </main>
    <footer class="mt-24 border-t border-line">
      <div class="mx-auto flex max-w-7xl flex-wrap items-center justify-between gap-4 px-4 py-10 text-sm text-muted">
        <AppLogo />
        <p>{{ $t('common.appTagline') }}</p>
        <LangSwitch />
      </div>
    </footer>
  </div>
</template>
