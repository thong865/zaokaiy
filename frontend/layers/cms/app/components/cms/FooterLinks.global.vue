<script setup lang="ts">
/** Footer column: news, guides and every policy (terms, privacy …). */
const { data: policies } = await useCmsList('legal')
</script>

<template>
  <div>
    <div class="eyebrow">{{ $t('cms.footer.title') }}</div>
    <ul class="mt-3 space-y-2 text-sm">
      <li><NuxtLink to="/news" class="text-ink/75 hover:text-brand-500">{{ $t('cms.section.news') }}</NuxtLink></li>
      <li><NuxtLink to="/guides" class="text-ink/75 hover:text-brand-500">{{ $t('cms.section.guides') }}</NuxtLink></li>
      <li v-for="p in policies" :key="p.path">
        <NuxtLink :to="p.path" class="text-ink/75 hover:text-brand-500">{{ p.title }}</NuxtLink>
      </li>
    </ul>
  </div>
</template>
