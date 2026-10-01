<script setup lang="ts">
/** Every policy (content/<lang>/legal/*.md): terms of service, privacy, returns, seller rules … */
const { t } = useI18n()
useSeoMeta({ title: () => t('cms.legal.title'), description: () => t('cms.legal.intro') })
const { data: policies } = await useCmsList('legal')
</script>

<template>
  <div class="mx-auto max-w-3xl px-4 py-10">
    <h1 class="page-title">{{ $t('cms.legal.title') }}</h1>
    <p class="mt-1 text-sm text-muted">{{ $t('cms.legal.intro') }}</p>
    <ul v-if="policies.length" class="card mt-6 divide-y divide-line/70">
      <li v-for="p in policies" :key="p.path">
        <NuxtLink :to="p.path" class="flex items-center gap-4 p-5 hover:bg-surface-2">
          <span class="icon-tile"><UIcon name="i-lucide-scale" class="size-5" /></span>
          <div class="min-w-0 flex-1">
            <div class="font-bold">{{ p.title }}</div>
            <div class="line-clamp-1 text-sm text-muted">{{ p.description || (p.updated && $t('cms.lastUpdated', { date: fmtDate(p.updated) })) }}</div>
          </div>
          <UIcon name="i-lucide-chevron-right" class="size-5 text-muted" />
        </NuxtLink>
      </li>
    </ul>
    <EmptyState v-else class="mt-6" icon="i-lucide-scale" :title="$t('cms.legal.empty')" />
  </div>
</template>
