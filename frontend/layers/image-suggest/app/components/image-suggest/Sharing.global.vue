<script setup lang="ts">
/** "Let other sellers use my product photos" switch (media library header). */
const props = defineProps<{ shopId: string }>()
const api = useApi()
interface Sharing { share: boolean; picked_by_others: number; available: boolean }
const { data, refresh } = useLazyAsyncData(
  () => `img-sharing:${props.shopId}`,
  () => (props.shopId ? api<Sharing>(`/shops/${props.shopId}/image-suggest/sharing`) : Promise.resolve(null)),
  { server: false, watch: [() => props.shopId] },
)
const busy = ref(false)
async function toggle(v: boolean) {
  busy.value = true
  try {
    data.value = await api<Sharing>(`/shops/${props.shopId}/image-suggest/sharing`, { method: 'PUT', body: { share: v } })
  } catch {
    await refresh()
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div v-if="data?.available" class="flex max-w-xl items-start gap-2 rounded-xl bg-[var(--field)] px-3 py-2">
    <USwitch :model-value="data.share" :disabled="busy" size="sm" class="mt-0.5" :aria-label="$t('imgsuggest.sharing.label')" @update:model-value="toggle" />
    <div class="text-xs">
      <div class="font-semibold">{{ $t('imgsuggest.sharing.label') }}</div>
      <div class="text-muted">{{ $t('imgsuggest.sharing.help') }}<template v-if="data.picked_by_others"> · {{ $t('imgsuggest.sharing.picked', { n: data.picked_by_others }, data.picked_by_others) }}</template></div>
    </div>
  </div>
</template>
