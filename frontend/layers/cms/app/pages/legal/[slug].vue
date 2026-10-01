<script setup lang="ts">
/** One policy: the other policies on the left, its headings on the right. */
const route = useRoute()
const { data } = await useCmsPageOr404(() => route.path)
const { data: policies } = await useCmsList('legal')
</script>

<template>
  <div class="mx-auto grid max-w-7xl gap-x-10 gap-y-6 px-4 py-10" :class="{ 'lg:grid-cols-[13rem_minmax(0,1fr)]': policies.length > 1 }">
    <aside v-if="policies.length > 1">
      <div class="lg:sticky lg:top-24">
        <div class="eyebrow">{{ $t('cms.legal.title') }}</div>
        <nav class="mt-3 flex gap-1 overflow-x-auto lg:flex-col">
          <NuxtLink
            v-for="p in policies"
            :key="p.path"
            :to="p.path"
            class="whitespace-nowrap rounded-xl px-3 py-2 text-sm font-semibold transition lg:whitespace-normal"
            :class="p.path === data?.page.path ? 'bg-brand-500/12 text-brand-600' : 'text-ink/75 hover:bg-surface-2'"
          >
            {{ p.title }}
          </NuxtLink>
        </nav>
      </div>
    </aside>
    <CmsArticleView v-if="data" :page="data.page" :fallback="data.fallback" />
  </div>
</template>
