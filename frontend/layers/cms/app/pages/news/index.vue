<script setup lang="ts">
/** News and announcements (content/<lang>/news/*.md), pinned first, then newest. */
const route = useRoute()
const { t } = useI18n()
useSeoMeta({ title: () => t('cms.news.title'), description: () => t('cms.news.intro') })

const { data: all } = await useCmsList('news')
const tag = computed(() => (route.query.tag as string) || '')
const items = computed(() => (tag.value ? all.value.filter((a) => a.tags?.includes(tag.value)) : all.value))
// Without a tag filter the first post is shown large on top.
const featured = computed(() => (tag.value ? null : items.value[0] ?? null))
const grid = computed(() => (featured.value ? items.value.slice(1) : items.value))
</script>

<template>
  <div class="mx-auto max-w-7xl px-4 py-10">
    <h1 class="page-title">{{ $t('cms.news.title') }}</h1>
    <p class="mt-1 text-sm text-muted">{{ $t('cms.news.intro') }}</p>
    <div v-if="tag" class="mt-3"><NuxtLink :to="{ query: {} }"><UBadge color="neutral" variant="soft" trailing-icon="i-lucide-x">#{{ tag }}</UBadge></NuxtLink></div>

    <template v-if="items.length">
      <CmsArticleCard v-if="featured" :item="featured" class="mt-6 sm:[&>div:first-child]:aspect-[21/9]" />
      <div class="mt-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <CmsArticleCard v-for="a in grid" :key="a.path" :item="a" />
      </div>
    </template>
    <EmptyState v-else class="mt-6" icon="i-lucide-newspaper" :title="$t('cms.news.empty')" />
  </div>
</template>
