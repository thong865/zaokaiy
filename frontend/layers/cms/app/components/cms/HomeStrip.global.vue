<script setup lang="ts">
/** Home page: latest news and the first guides. Hidden while both are empty. */
const { data: news } = await useCmsList('news', { limit: 3 })
const { data: guides } = await useCmsList('guides', { limit: 4 })
</script>

<template>
  <section v-if="news.length || guides.length" class="mx-auto max-w-7xl px-4 py-10">
    <template v-if="news.length">
      <div class="flex items-end justify-between gap-3">
        <h2 class="text-xl font-extrabold tracking-tight">{{ $t('cms.home.news') }}</h2>
        <NuxtLink to="/news" class="text-sm font-semibold text-brand-600 hover:underline">{{ $t('cms.home.all') }}</NuxtLink>
      </div>
      <div class="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <CmsArticleCard v-for="a in news" :key="a.path" :item="a" />
      </div>
    </template>
    <template v-if="guides.length">
      <div class="mt-10 flex items-end justify-between gap-3">
        <div>
          <h2 class="text-xl font-extrabold tracking-tight">{{ $t('cms.home.guides') }}</h2>
          <p class="text-sm text-muted">{{ $t('cms.home.guidesHint') }}</p>
        </div>
        <NuxtLink to="/guides" class="text-sm font-semibold text-brand-600 hover:underline">{{ $t('cms.home.all') }}</NuxtLink>
      </div>
      <div class="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <CmsArticleCard v-for="a in guides" :key="a.path" :item="a" compact />
      </div>
    </template>
  </section>
</template>
