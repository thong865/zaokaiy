<script setup lang="ts">
const { t, shop, device, status, lang, setLang, paired } = usePos()
const route = useRoute()
const nav = computed(() => [
  { to: '/', label: t('nav.till'), icon: 'i-lucide-shopping-cart' },
  { to: '/sales', label: t('nav.sales'), icon: 'i-lucide-receipt-text' },
  { to: '/settings', label: t('nav.settings'), icon: 'i-lucide-settings' },
])
const banner = computed(() => (status.value?.auth === 'unpaired' ? t('sync.unpaired') : status.value?.auth === 'forbidden' ? t('sync.forbidden') : ''))
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden bg-paper">
    <header v-if="!route.path.startsWith('/pair')" class="flex h-14 shrink-0 items-center gap-3 border-b border-line bg-surface px-4">
      <img src="/favicon.svg" alt="" class="size-8" />
      <div class="min-w-0 leading-tight">
        <div class="truncate text-sm font-bold">{{ shop?.name ?? t('app.name') }}</div>
        <div class="truncate text-[11px] text-muted">
          {{ device?.code }} · {{ device?.name }}<template v-if="device?.cashier?.name"> · {{ device.cashier.name }}</template>
        </div>
      </div>
      <nav class="ml-4 flex gap-1">
        <UButton
          v-for="n in nav"
          :key="n.to"
          :to="n.to"
          :icon="n.icon"
          :variant="route.path === n.to ? 'soft' : 'ghost'"
          :color="route.path === n.to ? 'primary' : 'neutral'"
          size="md"
        >{{ n.label }}</UButton>
      </nav>
      <div class="ml-auto flex items-center gap-2">
        <SyncPill v-if="paired" />
        <UButton variant="ghost" color="neutral" size="sm" @click="setLang(lang === 'en' ? 'lo' : 'en')">{{ lang === 'en' ? 'ລາວ' : 'EN' }}</UButton>
      </div>
    </header>
    <div v-if="banner && !route.path.startsWith('/pair')" class="flex items-center gap-3 bg-error/10 px-4 py-2 text-sm text-error">
      <UIcon name="i-lucide-shield-alert" class="size-4 shrink-0" />
      <span class="flex-1">{{ banner }}</span>
      <UButton size="xs" color="error" to="/pair">{{ t('sync.pairAgain') }}</UButton>
    </div>
    <main class="min-h-0 flex-1">
      <slot />
    </main>
  </div>
</template>
