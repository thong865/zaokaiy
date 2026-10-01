<script setup lang="ts">
import type { CmsItem } from '../../utils/cms'
import { SECTION_ICON, sectionOf } from '../../utils/cms'

/** News / guide tile for lists and the home strip. */
defineProps<{ item: CmsItem; compact?: boolean }>()
</script>

<template>
  <NuxtLink :to="item.path" class="card card-hover group flex flex-col overflow-hidden">
    <div v-if="!compact" class="aspect-[16/9] overflow-hidden bg-surface-2">
      <img v-if="item.cover" :src="item.cover" :alt="item.title" loading="lazy" class="size-full object-cover transition duration-500 group-hover:scale-105">
      <div v-else class="grid size-full place-items-center text-brand-500/40"><UIcon :name="SECTION_ICON[sectionOf(item.path)]" class="size-12" /></div>
    </div>
    <div class="flex flex-1 flex-col p-4">
      <div class="flex flex-wrap items-center gap-1.5 text-xs text-muted">
        <UBadge v-if="item.pinned" color="primary" variant="soft" size="sm" icon="i-lucide-pin">{{ $t('cms.pinned') }}</UBadge>
        <span v-if="item.date">{{ fmtDate(item.date) }}</span>
      </div>
      <h3 class="mt-1.5 line-clamp-2 font-bold leading-snug group-hover:text-brand-600">{{ item.title }}</h3>
      <p v-if="item.description" class="mt-1 line-clamp-2 text-sm text-muted">{{ item.description }}</p>
      <div v-if="item.author || item.tags?.length" class="mt-auto flex flex-wrap items-center gap-x-3 gap-y-1 pt-3 text-xs text-muted">
        <span v-if="item.author" class="truncate">{{ item.author }}</span>
        <span v-for="tag in (item.tags ?? []).slice(0, 3)" :key="tag">#{{ tag }}</span>
      </div>
    </div>
  </NuxtLink>
</template>
