<script setup lang="ts">
/** Modal preview of a private KYC document (image or PDF). */
const props = defineProps<{ docId: string; title: string }>()
const emit = defineEmits<{ close: [] }>()
const { url, type, loading, error, open } = useKybFile()
onMounted(() => open(props.docId))
</script>

<template>
  <UModal :open="true" :title="title" :ui="{ content: 'sm:max-w-4xl h-[85dvh]', body: 'p-0 sm:p-0 flex-1 grid place-items-center overflow-auto bg-muted' }" @update:open="(o) => !o && emit('close')">
    <template #actions>
      <UButton v-if="url" :to="url" target="_blank" rel="noopener" icon="i-lucide-external-link" color="neutral" variant="soft" size="sm">{{ $t('kyb.docs.openNew') }}</UButton>
    </template>
    <template #body>
      <p v-if="loading" class="text-sm text-muted">{{ $t('common.loading') }}</p>
      <p v-else-if="error" class="text-sm text-error" role="alert">{{ error }}</p>
      <img v-else-if="url && type.startsWith('image/')" :src="url" :alt="title" class="max-h-full max-w-full object-contain">
      <iframe v-else-if="url" :src="url" :title="title" class="size-full min-h-[70dvh] bg-white" />
    </template>
  </UModal>
</template>
