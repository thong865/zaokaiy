<script setup lang="ts">
import { en } from '@nuxt/ui/locale'
const head = useLocaleHead()
const { locale } = useI18n()
useHead({ htmlAttrs: { lang: () => head.value.htmlAttrs?.lang ?? 'en' } })
// Nuxt UI component texts (close, loading…) follow the UI language.
const uiLocale = computed(() => (locale.value === 'lo' ? { ...en, name: 'ລາວ', code: 'lo' } : en))
</script>

<template>
  <UApp :locale="uiLocale" :toaster="{ position: 'top-right' }" :tooltip="{ delayDuration: 200 }">
    <NuxtLoadingIndicator color="#ff4d2e" />
    <NuxtLayout>
      <NuxtPage />
    </NuxtLayout>
  </UApp>
</template>
