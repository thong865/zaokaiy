<script setup lang="ts">
import type { Settings } from '~/utils/types'
import { textCanvas } from '~/utils/receipt'

const { t, info, settings, saveSettings, shop, device, status, lang, setLang, refresh } = usePos()
const { printCanvas, openDrawer } = usePrinter()
const toast = useToast()

const form = reactive<Settings>(JSON.parse(JSON.stringify(settings.value)))
const printers = ref<string[]>([])
async function loadPrinters() {
  printers.value = await call<string[]>('list_printers').catch(() => [])
}
onMounted(loadPrinters)

let timer: ReturnType<typeof setTimeout> | undefined
watch(
  form,
  () => {
    clearTimeout(timer)
    timer = setTimeout(() => saveSettings(JSON.parse(JSON.stringify(form))), 300)
  },
  { deep: true },
)

const kinds = computed(() => (['none', 'network', 'system', 'window'] as const).map((k) => ({ label: t(`settings.kind.${k}`), value: k })))

async function run(fn: () => Promise<unknown>) {
  try {
    await saveSettings(JSON.parse(JSON.stringify(form)))
    await fn()
    toast.add({ title: t('receipt.printed'), color: 'success', icon: 'i-lucide-check' })
  } catch (e) {
    toast.add({ title: t('common.error'), description: (e as Error).message, color: 'error' })
  }
}
const testPrint = () => run(async () => {
  await document.fonts.ready
  await printCanvas(textCanvas(`${shop.value?.name ?? 'zaokaiy'}\n${t('settings.testText')}\nຂອບໃຈ · ขอบคุณ · Thank you`, form.printer.paper))
})

// ---- disconnect
const unpairOpen = ref(false)
const unpairError = ref('')
async function unpair(force: boolean) {
  unpairError.value = ''
  try {
    await call('unpair', { force })
    unpairOpen.value = false
    await refresh()
    await navigateTo('/pair')
  } catch (e) {
    unpairError.value = (e as Error).message
  }
}
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="mx-auto grid max-w-5xl grid-cols-2 gap-5">
      <h1 class="page-title col-span-2">{{ t('settings.title') }}</h1>

      <section class="card space-y-4 p-5">
        <h2 class="flex items-center gap-2 font-bold"><UIcon name="i-lucide-printer" class="size-5 text-brand-500" />{{ t('settings.printer') }}</h2>
        <URadioGroup v-model="form.printer.kind" :items="kinds" />
        <div v-if="form.printer.kind === 'network'" class="grid grid-cols-[1fr_6rem] gap-3">
          <UFormField :label="t('settings.host')"><UInput v-model="form.printer.host" placeholder="192.168.1.50" class="w-full" /></UFormField>
          <UFormField :label="t('settings.port')"><UInput v-model.number="form.printer.port" type="number" class="w-full" /></UFormField>
        </div>
        <div v-if="form.printer.kind === 'system'" class="flex items-end gap-2">
          <UFormField :label="t('settings.systemName')" class="flex-1">
            <USelect v-model="form.printer.name" :items="printers" class="w-full" />
          </UFormField>
          <UButton color="neutral" variant="soft" icon="i-lucide-refresh-cw" :title="t('settings.refresh')" @click="loadPrinters" />
        </div>
        <template v-if="form.printer.kind !== 'none'">
          <UFormField :label="t('settings.paper')">
            <UButtonGroup>
              <UButton :variant="form.printer.paper === 80 ? 'solid' : 'soft'" color="neutral" @click="form.printer.paper = 80">80 mm</UButton>
              <UButton :variant="form.printer.paper === 58 ? 'solid' : 'soft'" color="neutral" @click="form.printer.paper = 58">58 mm</UButton>
            </UButtonGroup>
          </UFormField>
          <USwitch v-model="form.printer.autoPrint" :label="t('settings.autoPrint')" />
          <template v-if="form.printer.kind !== 'window'">
            <USwitch v-model="form.printer.cut" :label="t('settings.cut')" />
            <USwitch v-model="form.printer.drawer" :label="t('settings.drawer')" />
          </template>
          <div class="flex gap-2">
            <UButton icon="i-lucide-printer" @click="testPrint">{{ t('settings.test') }}</UButton>
            <UButton v-if="form.printer.kind !== 'window'" color="neutral" variant="soft" icon="i-lucide-archive" @click="run(openDrawer)">{{ t('settings.openDrawer') }}</UButton>
          </div>
        </template>
      </section>

      <div class="space-y-5">
        <section class="card space-y-3 p-5">
          <h2 class="flex items-center gap-2 font-bold"><UIcon name="i-lucide-scan-barcode" class="size-5 text-brand-500" />{{ t('settings.scanner') }}</h2>
          <USwitch v-model="form.beep" :label="t('settings.beep')" />
        </section>
        <section class="card space-y-3 p-5">
          <h2 class="flex items-center gap-2 font-bold"><UIcon name="i-lucide-languages" class="size-5 text-brand-500" />{{ t('settings.language') }}</h2>
          <UButtonGroup>
            <UButton :variant="lang === 'en' ? 'solid' : 'soft'" color="neutral" @click="setLang('en')">English</UButton>
            <UButton :variant="lang === 'lo' ? 'solid' : 'soft'" color="neutral" @click="setLang('lo')">ລາວ</UButton>
          </UButtonGroup>
        </section>
        <section class="card p-5">
          <h2 class="mb-3 flex items-center gap-2 font-bold"><UIcon name="i-lucide-monitor-smartphone" class="size-5 text-brand-500" />{{ t('settings.terminal') }}</h2>
          <dl class="grid grid-cols-[9rem_1fr] gap-y-1.5 text-sm">
            <dt class="text-muted">{{ t('settings.shop') }}</dt><dd class="font-semibold">{{ shop?.name }}</dd>
            <dt class="text-muted">{{ t('settings.code') }}</dt><dd>{{ device?.code }} · {{ device?.name }}</dd>
            <dt class="text-muted">{{ t('settings.cashier') }}</dt><dd>{{ device?.cashier?.name }}</dd>
            <dt class="text-muted">{{ t('settings.products') }}</dt><dd>{{ status?.products }}</dd>
            <dt class="text-muted">{{ t('settings.server') }}</dt><dd class="truncate">{{ info?.api_base }}</dd>
            <dt class="text-muted">{{ t('settings.version') }}</dt><dd>{{ info?.version }} ({{ info?.os }})</dd>
          </dl>
          <UButton class="mt-4" color="error" variant="soft" icon="i-lucide-unplug" @click="unpairOpen = true">{{ t('settings.unpair') }}</UButton>
        </section>
      </div>
    </div>

    <UModal v-model:open="unpairOpen" :title="t('settings.unpair')">
      <template #body>
        <p class="text-sm">{{ t('settings.unpairConfirm') }}</p>
        <UAlert v-if="unpairError" class="mt-3" color="warning" variant="soft" :title="t('settings.unpairBlocked')" :description="unpairError" />
      </template>
      <template #footer>
        <div class="flex w-full flex-col gap-2">
          <UButton color="error" block @click="unpair(false)">{{ t('settings.unpair') }}</UButton>
          <UButton v-if="unpairError" color="error" variant="ghost" block size="xs" @click="unpair(true)">{{ t('settings.unpairForce') }}</UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
