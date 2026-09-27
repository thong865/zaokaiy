<script setup lang="ts">
const head = useLocaleHead()
useHead({ htmlAttrs: { lang: () => head.value.htmlAttrs?.lang ?? 'en' } })
</script>

<template>
  <NuxtLoadingIndicator color="#ff4d2e" />
  <NuxtLayout>
    <NuxtPage />
  </NuxtLayout>
</template>
