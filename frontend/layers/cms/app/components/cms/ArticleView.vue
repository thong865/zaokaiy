<script setup lang="ts">
import type { CmsEnCollectionItem } from '@nuxt/content'
import type { CmsSection } from '../../utils/cms'
import { readMinutes, sectionOf } from '../../utils/cms'

/**
 * A Markdown page: header, body (Nuxt UI prose — every heading has its own # link) and, on the
 * right, a navigator of its ## / ### headings that follows the scroll position.
 */
const props = defineProps<{ page: CmsEnCollectionItem; fallback?: boolean }>()
const { t } = useI18n()
const section = computed<CmsSection>(() => sectionOf(props.page.path))
const toc = computed(() => props.page.body?.toc?.links ?? [])
const tags = computed(() => props.page.tags ?? [])
const minutes = computed(() => readMinutes(props.page.body))
const changed = computed(() => props.page.updated || props.page.date)

useSeoMeta({
  title: () => props.page.title,
  description: () => props.page.description || undefined,
  ogTitle: () => props.page.title,
  ogDescription: () => props.page.description || undefined,
  ogImage: () => props.page.cover || undefined,
  ogType: 'article',
})

const copied = ref(false)
async function share() {
  const url = window.location.href
  if (navigator.share) return navigator.share({ title: props.page.title, url }).catch(() => {})
  await navigator.clipboard?.writeText(url)
  copied.value = true
  setTimeout(() => (copied.value = false), 1500)
}
</script>

<template>
  <div class="grid gap-x-10 lg:grid-cols-[minmax(0,1fr)_15rem]">
    <!-- Right-side navigator; a collapsible bar above the text on small screens -->
    <UContentToc
      v-if="toc.length"
      :links="toc"
      :title="t('cms.onThisPage')"
      highlight
      class="z-10 lg:col-start-2 lg:row-start-1 lg:self-start"
    />

    <article class="min-w-0 lg:col-start-1 lg:row-start-1">
      <nav class="flex items-center gap-1.5 text-sm text-muted">
        <NuxtLink :to="`/${section}`" class="hover:text-brand-600">{{ t(`cms.section.${section}`) }}</NuxtLink>
        <UIcon name="i-lucide-chevron-right" class="size-3.5 shrink-0" />
        <span class="truncate">{{ page.title }}</span>
      </nav>

      <header class="mt-4">
        <div v-if="tags.length && section !== 'legal'" class="flex flex-wrap gap-1.5">
          <NuxtLink v-for="tag in tags" :key="tag" :to="{ path: `/${section}`, query: { tag } }"><UBadge color="neutral" variant="soft">#{{ tag }}</UBadge></NuxtLink>
        </div>
        <h1 class="mt-2 text-3xl font-extrabold leading-tight tracking-tight sm:text-4xl">{{ page.title }}</h1>
        <p v-if="page.description" class="mt-3 text-lg text-muted">{{ page.description }}</p>
        <div class="mt-4 flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-muted">
          <span v-if="page.author" class="font-semibold text-ink">{{ page.author }}</span>
          <span v-if="section === 'legal' && changed">{{ t('cms.lastUpdated', { date: fmtDate(changed, 'long') }) }}</span>
          <span v-else-if="page.date">{{ fmtDate(page.date, 'long') }}</span>
          <span>{{ t('cms.readTime', { n: minutes }) }}</span>
          <UButton class="ml-auto" :icon="copied ? 'i-lucide-check' : 'i-lucide-share-2'" color="neutral" variant="ghost" size="sm" @click="share">
            {{ copied ? t('common.copied') : t('cms.share') }}
          </UButton>
        </div>
      </header>

      <p v-if="fallback" class="mt-6 flex items-center gap-2 rounded-xl bg-sun-400/15 p-2.5 text-xs text-amber-900 dark:text-amber-200">
        <UIcon name="i-lucide-languages" class="size-4 shrink-0" />{{ t('cms.notTranslated') }}
      </p>
      <img v-if="page.cover" :src="page.cover" :alt="page.title" class="mt-6 aspect-[2/1] w-full rounded-[var(--radius-card)] object-cover">

      <ContentRenderer :value="page" class="cms-body mt-8" />
    </article>
  </div>
</template>

<style scoped>
/* keep headings clear of the sticky site header when jumping from the navigator */
.cms-body :deep(:is(h2, h3, h4)) {
  scroll-margin-top: 6rem;
}
</style>
