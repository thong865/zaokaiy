<script setup lang="ts">
import type { ScanDiagnostic, ScannerDevice } from '#commerce/composables/useScannerDevices'

/** Barcode scanners of this till: names, live status, add / reconnect / remove, and a scan test. */
defineProps<{ devices: ScannerDevice[]; supported: { hid: boolean; serial: boolean }; diagnostic: ScanDiagnostic | null }>()
const emit = defineEmits<{ close: []; addHid: []; addSerial: [baud: number]; reconnect: [id: string]; disconnect: [id: string]; remove: [id: string]; rename: [id: string, name: string] }>()
const { t } = useI18n()
const baud = ref(9600)
const now = ref(Date.now())
onMounted(() => {
  const i = setInterval(() => (now.value = Date.now()), 5000)
  onBeforeUnmount(() => clearInterval(i))
})
const displayName = (d: ScannerDevice) => d.name || (d.kind === 'keyboard' ? t('pos.scanners.defaultName') : d.reported || t(`pos.scanners.kind.${d.kind}`))
function statusOf(d: ScannerDevice): { color: string; text: string } {
  if (d.status === 'error') return { color: 'bg-brand-500', text: t('pos.scanners.status.error') }
  if (d.kind === 'keyboard') {
    if (d.lastScanAt && now.value - d.lastScanAt < 10 * 60_000) return { color: 'bg-mint-500', text: t('pos.scanners.status.active', { ago: ago(new Date(d.lastScanAt).toISOString()) }) }
    return { color: 'bg-amber-500', text: t('pos.scanners.status.waiting') }
  }
  return d.status === 'connected'
    ? { color: 'bg-mint-500', text: t('pos.scanners.status.connected') }
    : { color: 'bg-line', text: t('pos.scanners.status.disconnected') }
}
const editing = ref<string | null>(null)
const draft = ref('')
function startEdit(d: ScannerDevice) {
  editing.value = d.id
  draft.value = d.name
}
function saveName() {
  if (editing.value) emit('rename', editing.value, draft.value)
  editing.value = null
}
</script>

<template>
  <UModal :open="true" :title="$t('pos.scanners.title')" :description="$t('pos.scanners.intro')" :ui="{ content: 'sm:max-w-2xl' }" @update:open="(o) => !o && emit('close')">
    <template #body>
      <div class="space-y-4 text-sm">
        <ul class="divide-y divide-line/70 rounded-2xl border border-line">
          <li v-for="d in devices" :key="d.id" class="flex flex-wrap items-center gap-3 p-3">
            <div class="grid size-10 shrink-0 place-items-center rounded-xl bg-[var(--field)]">
              <UIcon :name="d.kind === 'serial' ? 'i-lucide-cable' : d.kind === 'hid' ? 'i-lucide-usb' : 'i-lucide-scan-barcode'" class="size-5" />
            </div>
            <div class="min-w-0 flex-1">
              <form v-if="editing === d.id" class="flex gap-1" @submit.prevent="saveName">
                <UInput v-model="draft" size="sm" autofocus maxlength="60" class="flex-1" :placeholder="$t('pos.scanners.namePh')" :aria-label="$t('pos.scanners.rename')" />
                <UButton size="sm" type="submit">{{ $t('common.save') }}</UButton>
              </form>
              <div v-else class="flex items-center gap-1.5">
                <span class="truncate font-semibold">{{ displayName(d) }}</span>
                <button type="button" class="text-muted hover:text-ink" :aria-label="$t('pos.scanners.rename')" @click="startEdit(d)"><UIcon name="i-lucide-pencil" class="size-3.5" /></button>
              </div>
              <div class="text-xs text-muted">
                {{ $t(`pos.scanners.kind.${d.kind}`) }}<span v-if="d.reported && d.name && d.kind !== 'keyboard'"> · {{ d.reported }}</span><span v-if="d.baudRate && d.kind === 'serial'"> · {{ d.baudRate }} baud</span>
              </div>
              <div class="mt-0.5 flex items-center gap-1.5 text-xs">
                <span class="size-2 rounded-full" :class="statusOf(d).color" />
                <span>{{ statusOf(d).text }}</span>
                <span v-if="d.lastCode" class="font-mono text-muted">· {{ d.lastCode }}</span>
              </div>
              <p v-if="d.error" class="mt-1 text-xs text-brand-700">{{ d.error }}</p>
            </div>
            <div v-if="d.kind !== 'keyboard'" class="flex gap-1">
              <UButton v-if="d.status !== 'connected'" size="xs" color="neutral" variant="soft" icon="i-lucide-plug" @click="emit('reconnect', d.id)">{{ $t('pos.scanners.connect') }}</UButton>
              <UButton v-else size="xs" color="neutral" variant="ghost" icon="i-lucide-unplug" @click="emit('disconnect', d.id)">{{ $t('pos.scanners.disconnect') }}</UButton>
              <UButton size="xs" color="error" variant="ghost" icon="i-lucide-trash-2" :aria-label="$t('common.remove')" @click="emit('remove', d.id)" />
            </div>
          </li>
        </ul>

        <!-- Add a device -->
        <div class="grid gap-2 sm:grid-cols-2">
          <div class="rounded-2xl border border-line p-3">
            <div class="font-semibold">{{ $t('pos.scanners.addHid') }}</div>
            <p class="mt-0.5 text-xs text-muted">{{ $t('pos.scanners.addHidHelp') }}</p>
            <UButton class="mt-2" size="sm" icon="i-lucide-usb" :disabled="!supported.hid" @click="emit('addHid')">{{ $t('pos.scanners.choose') }}</UButton>
          </div>
          <div class="rounded-2xl border border-line p-3">
            <div class="font-semibold">{{ $t('pos.scanners.addSerial') }}</div>
            <p class="mt-0.5 text-xs text-muted">{{ $t('pos.scanners.addSerialHelp') }}</p>
            <div class="mt-2 flex gap-2">
              <select v-model.number="baud" class="input w-28 py-1.5 text-xs" :aria-label="$t('pos.scanners.baud')"><option v-for="b in [9600, 19200, 38400, 57600, 115200]" :key="b" :value="b">{{ b }}</option></select>
              <UButton size="sm" icon="i-lucide-cable" :disabled="!supported.serial" @click="emit('addSerial', baud)">{{ $t('pos.scanners.choose') }}</UButton>
            </div>
          </div>
        </div>
        <p v-if="!supported.hid || !supported.serial" class="text-xs text-muted">{{ $t('pos.scanners.unsupported') }}</p>

        <!-- Test: what the last keyboard-mode scan looked like -->
        <div class="rounded-2xl bg-[var(--field)] p-3 text-xs">
          <div class="font-bold">{{ $t('pos.scan.testTitle') }}</div>
          <template v-if="diagnostic">
            <dl class="mt-2 grid grid-cols-[8rem_1fr] gap-x-2 gap-y-0.5">
              <dt class="text-muted">{{ $t('pos.scan.diag.code') }}</dt><dd class="break-all font-mono font-bold">{{ diagnostic.code }}</dd>
              <template v-if="diagnostic.typed !== diagnostic.code"><dt class="text-muted">{{ $t('pos.scan.diag.typed') }}</dt><dd class="break-all font-mono">{{ diagnostic.typed }}</dd></template>
              <dt class="text-muted">{{ $t('pos.scan.diag.speed') }}</dt><dd>{{ $t('pos.scan.diag.speedValue', { n: diagnostic.keys, median: diagnostic.medianGapMs, max: diagnostic.maxGapMs }) }}</dd>
              <dt class="text-muted">{{ $t('pos.scan.diag.ended') }}</dt><dd>{{ $t(`pos.scan.diag.end.${diagnostic.ended}`) }}</dd>
              <dt class="text-muted">{{ $t('pos.scan.diag.result') }}</dt>
              <dd :class="diagnostic.accepted ? 'font-bold text-mint-500' : 'font-bold text-brand-700'">{{ diagnostic.accepted ? $t('pos.scan.diag.ok') : $t(`pos.scan.diag.why.${diagnostic.reason}`) }}</dd>
            </dl>
            <p v-if="diagnostic.typed !== diagnostic.code" class="mt-2 text-muted">{{ $t('pos.scan.diag.layout') }}</p>
          </template>
          <p v-else class="mt-2 text-muted">{{ $t('pos.scan.diag.waiting') }}</p>
          <ul class="mt-2 list-disc space-y-0.5 pl-4 text-muted">
            <li>{{ $t('pos.scan.diag.tipNotepad') }}</li>
            <li>{{ $t('pos.scan.diag.tipHid') }}</li>
            <li>{{ $t('pos.scan.diag.tipSuffix') }}</li>
          </ul>
        </div>
      </div>
    </template>
  </UModal>
</template>
