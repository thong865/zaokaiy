<script setup lang="ts">
const route = useRoute()
const { data } = await useCmsPageOr404(() => route.path)
const { data: latest } = await useCmsList('news', { limit: 4 })
const others = computed(() => latest.value.filter((a) => a.path !== data.value?.page.path).slice(0, 3))
</script>

<template>
  <div class="mx-auto max-w-6xl px-4 py-10">
    <CmsArticleView v-if="data" :page="data.page" :fallback="data.fallback" />
    <section v-if="others.length" class="mt-14">
      <h2 class="text-lg font-extrabold">{{ $t('cms.news.more') }}</h2>
      <div class="mt-4 grid gap-4 sm:grid-cols-3">
        <CmsArticleCard v-for="a in others" :key="a.path" :item="a" compact />
      </div>
    </section>
  </div>
</template>
