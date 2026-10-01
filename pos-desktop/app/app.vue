<script setup lang="ts">
const { refresh, startListening, paired } = usePos()
const ready = ref(false)
const route = useRoute()

onMounted(async () => {
  try {
    await refresh()
    await startListening()
  } finally {
    ready.value = true
  }
  if (!paired.value && route.path !== '/pair') await navigateTo('/pair')
})
// Block the webview's own shortcuts that make no sense in a till (reload, find, print page…).
onMounted(() => {
  window.addEventListener('contextmenu', (e) => {
    if (!(e.target as HTMLElement)?.closest('input, textarea')) e.preventDefault()
  })
  window.addEventListener('keydown', (e) => {
    if ((e.key === 'F5' || (e.ctrlKey && ['r', 'p', 'f'].includes(e.key.toLowerCase()))) && !import.meta.dev) e.preventDefault()
  })
})
</script>

<template>
  <UApp :toaster="{ position: 'bottom-right', duration: 2500 }">
    <div v-if="!ready" class="grid h-screen place-items-center">
      <UIcon name="i-lucide-loader-circle" class="size-8 animate-spin text-muted" />
    </div>
    <NuxtLayout v-else>
      <NuxtPage />
    </NuxtLayout>
  </UApp>
</template>
