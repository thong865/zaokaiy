<script setup lang="ts">
const route = useRoute()
const { data } = await useCmsPageOr404(() => route.path)
const { data: all } = await useCmsList('guides')
// Related: guides sharing a tag, otherwise the next ones in the list.
const others = computed(() => {
  const cur = data.value?.page
  const rest = all.value.filter((a) => a.path !== cur?.path)
  const related = rest.filter((a) => a.tags?.some((tg) => cur?.tags?.includes(tg)))
  return (related.length ? related : rest).slice(0, 3)
})
</script>

<template>
  <div class="mx-auto max-w-6xl px-4 py-10">
    <CmsArticleView v-if="data" :page="data.page" :fallback="data.fallback" />
    <section v-if="others.length" class="mt-14">
      <h2 class="text-lg font-extrabold">{{ $t('cms.guides.related') }}</h2>
      <div class="mt-4 grid gap-4 sm:grid-cols-3">
        <CmsArticleCard v-for="a in others" :key="a.path" :item="a" compact />
      </div>
    </section>
  </div>
</template>
