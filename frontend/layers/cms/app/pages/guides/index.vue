<script setup lang="ts">
/** How-to guides (content/<lang>/guides/*.md), searchable and filterable by tag. */
const route = useRoute()
const router = useRouter()
const { t } = useI18n()
useSeoMeta({ title: () => t('cms.guides.title'), description: () => t('cms.guides.intro') })

const { data: all } = await useCmsList('guides')
const q = ref((route.query.q as string) || '')
const tag = computed(() => (route.query.tag as string) || '')
const tags = computed(() => [...new Set(all.value.flatMap((a) => a.tags ?? []))].sort())
const items = computed(() => {
  const words = ((route.query.q as string) || '').toLowerCase().split(/\s+/).filter(Boolean)
  return all.value.filter((a) => {
    if (tag.value && !a.tags?.includes(tag.value)) return false
    const text = `${a.title} ${a.description} ${(a.tags ?? []).join(' ')}`.toLowerCase()
    return words.every((w) => text.includes(w))
  })
})
const search = () => router.replace({ query: { ...route.query, q: q.value.trim() || undefined } })
</script>

<template>
  <div class="mx-auto max-w-7xl px-4 py-10">
    <div class="card relative overflow-hidden p-6 sm:p-10">
      <div class="absolute -right-10 -top-10 size-48 rounded-full bg-brand-500/10" />
      <div class="relative max-w-2xl">
        <div class="eyebrow">{{ $t('cms.guides.eyebrow') }}</div>
        <h1 class="mt-1 text-3xl font-extrabold tracking-tight">{{ $t('cms.guides.title') }}</h1>
        <p class="mt-2 text-muted">{{ $t('cms.guides.intro') }}</p>
        <form class="mt-5 flex gap-2" role="search" @submit.prevent="search">
          <UInput v-model="q" icon="i-lucide-search" size="lg" class="flex-1" :placeholder="$t('cms.guides.searchPh')" :aria-label="$t('common.search')" />
          <UButton type="submit" size="lg">{{ $t('common.search') }}</UButton>
        </form>
      </div>
    </div>

    <div v-if="tags.length" class="mt-6 flex flex-wrap items-center gap-2">
      <NuxtLink :to="{ query: { ...route.query, tag: undefined } }"><UBadge :color="tag ? 'neutral' : 'primary'" variant="soft">{{ $t('cms.guides.allTopics') }}</UBadge></NuxtLink>
      <NuxtLink v-for="tg in tags" :key="tg" :to="{ query: { ...route.query, tag: tg } }"><UBadge :color="tag === tg ? 'primary' : 'neutral'" variant="soft">#{{ tg }}</UBadge></NuxtLink>
    </div>

    <div v-if="items.length" class="mt-5 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <CmsArticleCard v-for="a in items" :key="a.path" :item="a" />
    </div>
    <EmptyState v-else class="mt-5" icon="i-lucide-book-open" :title="$t('cms.guides.empty')" />
  </div>
</template>
