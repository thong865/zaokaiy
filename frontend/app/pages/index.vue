<script setup lang="ts">
/**
 * Super-app home: hero, one tile per core (shop, vehicles, food, insurance …) and each core's
 * home section. Tiles and sections come from the layers (see composables/useCores.ts).
 */
const route = useRoute()
const searching = computed(() => !!route.query.q || !!route.query.category)
const { loggedIn } = useAuth()
const { tiles, homeSections } = useCores()
</script>

<template>
  <div>
    <section v-if="!searching" class="mx-auto max-w-7xl px-4 pt-10">
      <div class="relative overflow-hidden rounded-[28px] bg-night px-6 py-14 text-white shadow-[0_20px_50px_-20px_rgb(16_14_12/0.6)] sm:px-12 sm:py-20">
        <div class="absolute -right-24 -top-28 size-[28rem] rounded-full bg-brand-500/45 blur-3xl" />
        <div class="absolute -bottom-40 left-1/4 size-96 rounded-full bg-violet-500/25 blur-3xl" />
        <div class="absolute bottom-0 right-1/4 size-64 rounded-full bg-sun-400/15 blur-3xl" />
        <div class="absolute inset-0 bg-[radial-gradient(rgb(255_255_255/0.07)_1px,transparent_1px)] [background-size:22px_22px]" />
        <div class="relative max-w-2xl">
          <span class="chip gap-1.5 rounded-full bg-white/10 px-3 py-1 text-white backdrop-blur"><AppIcon name="sparkles" class="size-3.5 text-sun-400" />{{ $t('store.home.heroChip') }}</span>
          <h1 class="mt-5 text-4xl font-extrabold leading-[1.05] tracking-tight sm:text-6xl">
            {{ $t('store.home.heroTitle1') }}<br>{{ $t('store.home.heroTitle2') }} <span class="bg-gradient-to-r from-brand-500 to-sun-400 bg-clip-text text-transparent">{{ $t('store.home.heroTitle3') }}</span>
          </h1>
          <p class="mt-5 max-w-lg text-white/70">
            {{ $t('store.home.heroText') }}
          </p>
          <div class="mt-8 flex flex-wrap gap-3">
            <UButton size="xl" :to="loggedIn ? '/dashboard' : '/register'">{{ $t('store.home.startSelling') }} <AppIcon name="chevron-right" class="size-4" /></UButton>
            <UButton color="neutral" variant="ghost" size="xl" :to="loggedIn ? '/dashboard/marketplace' : '/register'" class="bg-white/10 text-white backdrop-blur hover:bg-white/20">{{ $t('store.home.becomeSellStaff') }}</UButton>
          </div>
        </div>
      </div>
      <div class="mt-6 grid gap-4 sm:grid-cols-3">
        <div v-for="(f, i) in ['ownShop', 'network', 'ai']" :key="f" class="card card-hover flex gap-4 p-5">
          <span class="icon-tile" :class="['bg-brand-500/12 text-brand-600', 'bg-mint-500/12 text-mint-500', 'bg-violet-500/12 text-violet-500'][i]"><AppIcon :name="['store', 'users', 'sparkles'][i]!" class="size-5" /></span>
          <div>
            <div class="font-bold">{{ $t(`store.home.features.${f}.title`) }}</div>
            <div class="mt-1 text-sm text-muted">{{ $t(`store.home.features.${f}.text`) }}</div>
          </div>
        </div>
      </div>
    </section>

    <section v-if="!searching && tiles.length" class="mx-auto max-w-7xl px-4 pt-8">
      <h2 class="sr-only">{{ $t('common.superApp.services') }}</h2>
      <div class="grid grid-cols-2 gap-3 sm:grid-cols-4" data-testid="core-tiles">
        <NuxtLink v-for="t in tiles" :key="t.name" :to="t.to" class="card card-hover flex items-center gap-3 p-4">
          <span class="icon-tile" :class="t.color"><UIcon :name="`i-lucide-${t.icon}`" class="size-5" /></span>
          <span class="min-w-0">
            <span class="block font-bold">{{ $t(t.label) }}</span>
            <span class="block truncate text-xs text-muted">{{ $t(t.hint) }}</span>
          </span>
        </NuxtLink>
      </div>
    </section>
    <component :is="s.component" v-for="(s, i) in homeSections" :key="i" />
    <ModuleSlot name="home.sections" />
  </div>
</template>
