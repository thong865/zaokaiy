<script setup lang="ts">
import type { SaleRecord } from '~/utils/types'

const props = defineProps<{ sale: SaleRecord | null }>()
const open = defineModel<boolean>('open', { default: false })
const emit = defineEmits<{ next: [] }>()
const { t, currency, status } = usePos()
const { enabled, printSale } = usePrinter()
const toast = useToast()
const printing = ref(false)

async function print(reprint = false) {
  if (!props.sale) return
  printing.value = true
  try {
    await printSale(props.sale, { reprint })
    toast.add({ title: t('receipt.printed'), icon: 'i-lucide-printer', color: 'success' })
  } catch (e) {
    toast.add({ title: t('common.error'), description: (e as Error).message, color: 'error' })
  } finally {
    printing.value = false
  }
}
defineExpose({ print })

function onKey(e: KeyboardEvent) {
  if (open.value && e.key === 'Enter') {
    e.preventDefault()
    open.value = false
    emit('next')
  }
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <UModal v-model:open="open" :title="t('receipt.title')" :ui="{ content: 'max-w-lg' }">
    <template #body>
      <div v-if="sale" class="space-y-4">
        <div class="flex items-end justify-between">
          <div>
            <div class="text-sm text-muted">{{ sale.number }}</div>
            <div class="text-3xl font-extrabold tabular-nums">{{ money(sale.total_cents, currency) }}</div>
          </div>
          <div v-if="sale.sale.change_cents" class="text-right">
            <div class="text-sm text-muted">{{ t('receipt.change') }}</div>
            <div class="text-3xl font-extrabold text-mint-500 tabular-nums">{{ money(sale.sale.change_cents, currency) }}</div>
          </div>
        </div>
        <UAlert v-if="!status?.online" color="warning" variant="soft" icon="i-lucide-wifi-off" :description="t('receipt.offline')" />
        <ReceiptPreview :sale="sale" />
      </div>
    </template>
    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton v-if="enabled" color="neutral" variant="soft" icon="i-lucide-printer" :loading="printing" @click="print(true)">{{ t('receipt.printAgain') }}</UButton>
        <UButton size="lg" icon="i-lucide-plus" @click="open = false; emit('next')">{{ t('receipt.newSale') }} ⏎</UButton>
      </div>
    </template>
  </UModal>
</template>
