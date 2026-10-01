<script setup lang="ts">
/** Receipt printer of this till: default printer, paper, copies, auto-print after checkout. */
const emit = defineEmits<{ close: [] }>()
const { settings, status, error, directSupported, displayName, connectDirect, disconnectDirect, testPrint } = usePosPrinter()
const { shop } = useShop()
const { t } = useI18n()
const statusText = computed(() =>
  settings.value.mode === 'browser' ? t('pos.printer.status.browser') : t(`pos.printer.status.${status.value}`),
)
const statusColor = computed(() =>
  settings.value.mode === 'browser' || status.value === 'connected' ? 'bg-mint-500' : status.value === 'printing' ? 'bg-amber-500' : status.value === 'error' ? 'bg-brand-500' : 'bg-line',
)
const kioskCmd = '"C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe" --kiosk-printing'
const copied = ref(false)
async function copyCmd() {
  try {
    await navigator.clipboard.writeText(kioskCmd)
    copied.value = true
  } catch {}
}
</script>

<template>
  <UModal :open="true" :title="$t('pos.printer.title')" :description="$t('pos.printer.intro')" :ui="{ content: 'sm:max-w-2xl' }" @update:open="(o) => !o && emit('close')">
    <template #body>
      <div class="space-y-4 text-sm">
        <!-- Default printer -->
        <div class="flex flex-wrap items-center gap-3 rounded-2xl border border-line p-3">
          <div class="grid size-10 place-items-center rounded-xl bg-[var(--field)]"><UIcon name="i-lucide-printer" class="size-5" /></div>
          <div class="min-w-0 flex-1">
            <div class="font-semibold">{{ displayName }}</div>
            <div class="flex items-center gap-1.5 text-xs"><span class="size-2 rounded-full" :class="statusColor" />{{ statusText }}</div>
            <p v-if="error" class="mt-1 text-xs text-brand-700">{{ error }}</p>
          </div>
          <UButton size="sm" color="neutral" icon="i-lucide-file-check" :disabled="settings.mode === 'direct' && status !== 'connected'" @click="testPrint({ name: shop?.name ?? 'zaokaiy', currency: shop?.currency ?? 'LAK' })">{{ $t('pos.printer.test') }}</UButton>
        </div>

        <div>
          <label class="label" for="pr-name">{{ $t('pos.printer.name') }}</label>
          <UInput id="pr-name" v-model="settings.name" maxlength="60" class="w-full" :placeholder="$t('pos.printer.namePh')" />
        </div>

        <!-- How to print -->
        <fieldset class="grid gap-2 sm:grid-cols-2">
          <legend class="label">{{ $t('pos.printer.mode') }}</legend>
          <label class="cursor-pointer rounded-2xl border p-3" :class="settings.mode === 'browser' ? 'border-brand-500 bg-brand-50/50' : 'border-line'">
            <div class="flex items-center gap-2 font-semibold"><input v-model="settings.mode" type="radio" value="browser" class="accent-brand-500">{{ $t('pos.printer.browser') }}</div>
            <p class="mt-1 text-xs text-muted">{{ $t('pos.printer.browserHelp') }}</p>
          </label>
          <label class="cursor-pointer rounded-2xl border p-3" :class="[settings.mode === 'direct' ? 'border-brand-500 bg-brand-50/50' : 'border-line', !directSupported && 'opacity-60']">
            <div class="flex items-center gap-2 font-semibold"><input v-model="settings.mode" type="radio" value="direct" class="accent-brand-500" :disabled="!directSupported">{{ $t('pos.printer.direct') }}</div>
            <p class="mt-1 text-xs text-muted">{{ directSupported ? $t('pos.printer.directHelp') : $t('pos.printer.directUnsupported') }}</p>
          </label>
        </fieldset>

        <div v-if="settings.mode === 'direct'" class="flex flex-wrap items-end gap-2 rounded-2xl bg-[var(--field)] p-3">
          <div>
            <label class="label" for="pr-baud">{{ $t('pos.scanners.baud') }}</label>
            <select id="pr-baud" v-model.number="settings.baudRate" class="input w-28 py-1.5 text-xs"><option v-for="b in [9600, 19200, 38400, 57600, 115200]" :key="b" :value="b">{{ b }}</option></select>
          </div>
          <UButton v-if="status !== 'connected'" icon="i-lucide-plug" @click="connectDirect">{{ $t('pos.printer.choose') }}</UButton>
          <UButton v-else color="neutral" variant="soft" icon="i-lucide-unplug" @click="disconnectDirect">{{ $t('pos.scanners.disconnect') }}</UButton>
          <label class="ml-auto flex items-center gap-2 text-xs"><input v-model="settings.drawer" type="checkbox" class="size-4 accent-brand-500">{{ $t('pos.printer.drawer') }}</label>
        </div>

        <div v-else class="rounded-2xl bg-[var(--field)] p-3 text-xs">
          <div class="font-semibold">{{ $t('pos.printer.kioskTitle') }}</div>
          <p class="mt-1 text-muted">{{ $t('pos.printer.kioskHelp') }}</p>
          <div class="mt-2 flex items-center gap-2">
            <code class="min-w-0 flex-1 break-all rounded-lg bg-surface px-2 py-1 font-mono text-[11px]">{{ kioskCmd }}</code>
            <UButton size="xs" color="neutral" :icon="copied ? 'i-lucide-check' : 'i-lucide-copy'" @click="copyCmd">{{ copied ? $t('common.copied') : $t('common.copy') }}</UButton>
          </div>
        </div>

        <!-- Receipt options -->
        <div class="grid gap-3 sm:grid-cols-2">
          <div>
            <label class="label" for="pr-paper">{{ $t('pos.paperWidth') }}</label>
            <select id="pr-paper" v-model="settings.paper" class="input"><option value="80">80 mm</option><option value="58">58 mm</option></select>
          </div>
          <div>
            <label class="label" for="pr-copies">{{ $t('pos.printer.copies') }}</label>
            <select id="pr-copies" v-model.number="settings.copies" class="input"><option v-for="n in [1, 2, 3]" :key="n" :value="n">{{ n }}</option></select>
          </div>
        </div>
        <label class="flex items-start gap-2"><input v-model="settings.autoPrint" type="checkbox" class="mt-0.5 size-4 accent-brand-500"><span><span class="font-semibold">{{ $t('pos.printer.auto') }}</span><span class="block text-xs text-muted">{{ $t('pos.printer.autoHelp') }}</span></span></label>
        <label class="flex items-start gap-2"><input v-model="settings.autoInvoice" type="checkbox" class="mt-0.5 size-4 accent-brand-500"><span><span class="font-semibold">{{ $t('pos.printer.autoInvoice') }}</span><span class="block text-xs text-muted">{{ $t('pos.printer.autoInvoiceHelp') }}</span></span></label>
      </div>
    </template>
  </UModal>
</template>
