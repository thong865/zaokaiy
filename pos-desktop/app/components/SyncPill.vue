<script setup lang="ts">
const { status, t, lang, syncNow } = usePos()
const busy = ref(false)
const tick = ref(0)
let timer: ReturnType<typeof setInterval> | undefined
onMounted(() => (timer = setInterval(() => tick.value++, 30_000)))
onBeforeUnmount(() => clearInterval(timer))

const view = computed(() => {
  const s = status.value
  if (!s) return { color: 'neutral' as const, icon: 'i-lucide-circle', label: '' }
  if (s.syncing || busy.value) return { color: 'info' as const, icon: 'i-lucide-refresh-cw', label: t('sync.syncing'), spin: true }
  if (s.rejected > 0) return { color: 'error' as const, icon: 'i-lucide-triangle-alert', label: t('sync.attention', { n: s.rejected }) }
  if (!s.online) return { color: 'warning' as const, icon: 'i-lucide-wifi-off', label: s.pending ? t('sync.waiting', { n: s.pending }) : t('sync.offline') }
  if (s.pending > 0) return { color: 'info' as const, icon: 'i-lucide-cloud-upload', label: t('sync.waiting', { n: s.pending }) }
  return { color: 'success' as const, icon: 'i-lucide-cloud-check', label: t('sync.synced') }
})
const last = computed(() => {
  void tick.value
  return status.value?.last_sync_at ? t('sync.last', { when: ago(status.value.last_sync_at, lang.value) }) : t('sync.never')
})

async function now() {
  busy.value = true
  try {
    await syncNow()
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UPopover :content="{ align: 'end' }">
    <UButton :color="view.color" variant="soft" size="sm" :aria-label="view.label">
      <UIcon :name="view.icon" :class="['size-4', view.spin && 'animate-spin']" />
      <span class="max-w-44 truncate">{{ view.label }}</span>
    </UButton>
    <template #content>
      <div class="w-72 space-y-3 p-4 text-sm">
        <div class="flex items-center gap-2 font-semibold">
          <span :class="['size-2.5 rounded-full', status?.online ? 'bg-mint-500' : 'bg-amber-500']" />
          {{ status?.online ? t('sync.online') : t('sync.offline') }}
        </div>
        <p class="text-muted">{{ last }}</p>
        <p v-if="!status?.online" class="text-muted">{{ t('sync.offlineHelp') }}</p>
        <p v-if="status?.last_error" class="text-error">{{ status.last_error }}</p>
        <UButton block icon="i-lucide-refresh-cw" :loading="busy || status?.syncing" @click="now">{{ t('sync.now') }}</UButton>
      </div>
    </template>
  </UPopover>
</template>
