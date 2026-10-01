<script setup lang="ts">
import type { SaleRecord } from '~/utils/types'
import { summaryCanvas, type Summary } from '~/utils/receipt'

const { t, lang, currency, device, shop, dataVersion, settings } = usePos()
const { enabled: printerOn, printSale, printCanvas } = usePrinter()
const toast = useToast()

const day = ref(todayYmd())
const q = ref('')
const filter = ref<'' | 'pending' | 'rejected'>('')
const sales = ref<SaleRecord[]>([])
const summary = ref<Summary | null>(null)
const selected = ref<SaleRecord | null>(null)

async function load() {
  const [from, to] = dayRange(day.value)
  const all = !!filter.value // "waiting" / "needs attention" look at every day
  sales.value = await call<SaleRecord[]>('list_sales', { from: all ? null : from, to: all ? null : to, q: q.value || null, state: filter.value || null, limit: 500 })
  summary.value = await call<Summary>('sales_summary', { from, to })
  if (selected.value) selected.value = sales.value.find((s) => s.id === selected.value!.id) ?? (await call<SaleRecord | null>('get_sale', { id: selected.value.id }))
}
watch([day, filter, dataVersion], load)
let timer: ReturnType<typeof setTimeout> | undefined
watch(q, () => {
  clearTimeout(timer)
  timer = setTimeout(load, 200)
})
onMounted(load)

const methodsOf = (s: SaleRecord) => [...new Set(s.sale.payments.map((p) => t(`pay.method.${p.method}`)))].join(' + ')
function syncBadge(s: SaleRecord) {
  if (s.sync_state === 'rejected' || s.void_sync === 'rejected') return { color: 'error' as const, label: t('sales.st.rejected'), icon: 'i-lucide-triangle-alert' }
  if (s.sync_state === 'pending' || s.void_sync === 'pending') return { color: 'warning' as const, label: t('sales.st.pending'), icon: 'i-lucide-clock' }
  return { color: 'success' as const, label: t('sales.st.synced'), icon: 'i-lucide-cloud-check' }
}

// ---- actions
const voidOpen = ref(false)
const voidReason = ref('')
const busy = ref(false)
async function doVoid() {
  if (!selected.value) return
  busy.value = true
  try {
    selected.value = await call<SaleRecord>('void_sale', { id: selected.value.id, reason: voidReason.value })
    voidOpen.value = false
    voidReason.value = ''
    await load()
  } catch (e) {
    toast.add({ title: t('common.error'), description: (e as Error).message, color: 'error' })
  } finally {
    busy.value = false
  }
}
async function retry(s: SaleRecord) {
  await call('retry_sale', { id: s.id })
  await load()
}
async function reprint(s: SaleRecord) {
  try {
    await printSale(s, { reprint: true })
    toast.add({ title: t('receipt.printed'), icon: 'i-lucide-printer', color: 'success' })
  } catch (e) {
    toast.add({ title: t('common.error'), description: (e as Error).message, color: 'error' })
  }
}
async function printSummary() {
  if (!summary.value) return
  try {
    await document.fonts.ready
    await printCanvas(summaryCanvas(summary.value, day.value, shop.value?.name ?? '', device.value?.code ?? '', currency.value, lang.value, settings.value.printer.paper))
  } catch (e) {
    toast.add({ title: t('common.error'), description: (e as Error).message, color: 'error' })
  }
}
</script>

<template>
  <div class="grid h-full grid-cols-[1fr_24rem] overflow-hidden">
    <section class="flex min-h-0 flex-col gap-4 p-5">
      <div class="flex items-center gap-2">
        <h1 class="page-title mr-auto">{{ t('sales.title') }}</h1>
        <UInput v-model="day" type="date" class="w-44" :disabled="!!filter" />
        <UButton color="neutral" variant="soft" :disabled="day === todayYmd()" @click="day = todayYmd()">{{ t('sales.today') }}</UButton>
        <UInput v-model="q" icon="i-lucide-search" :placeholder="t('sales.search')" class="w-44" />
      </div>

      <div v-if="summary" class="grid grid-cols-4 gap-3">
        <div class="card p-4">
          <div class="eyebrow">{{ t('sales.count') }}</div>
          <div class="text-2xl font-extrabold tabular-nums">{{ summary.count }}</div>
        </div>
        <div class="card p-4">
          <div class="eyebrow">{{ t('sales.takings') }}</div>
          <div class="text-2xl font-extrabold tabular-nums">{{ money(summary.total_cents, currency) }}</div>
        </div>
        <div class="card p-4">
          <div class="eyebrow">{{ t('sales.byMethod') }}</div>
          <div class="mt-1 space-y-0.5 text-xs">
            <div v-for="(v, k) in summary.methods" :key="k" class="flex justify-between"><span>{{ t(`pay.method.${k}`) }}</span><span class="tabular-nums">{{ money(v, currency) }}</span></div>
          </div>
        </div>
        <div class="card flex flex-col p-4">
          <div class="eyebrow">{{ t('sales.voids') }}</div>
          <div class="text-2xl font-extrabold tabular-nums">{{ summary.voided }}</div>
          <UButton v-if="printerOn" size="xs" variant="soft" class="mt-auto self-start" icon="i-lucide-printer" @click="printSummary">{{ t('sales.zreport') }}</UButton>
        </div>
      </div>

      <UTabs
        v-model="filter"
        :items="[{ label: t('sales.all'), value: '' }, { label: t('sales.waiting'), value: 'pending' }, { label: t('sales.attention'), value: 'rejected' }]"
        :content="false"
        size="sm"
        class="w-96"
      />

      <div class="card min-h-0 flex-1 overflow-auto">
        <table class="table">
          <thead class="sticky top-0 bg-surface">
            <tr>
              <th>{{ t('sales.number') }}</th>
              <th>{{ t('sales.time') }}</th>
              <th>{{ t('sales.payment') }}</th>
              <th class="text-right">{{ t('sales.total') }}</th>
              <th>{{ t('sales.state') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="s in sales" :key="s.id" :class="['cursor-pointer', selected?.id === s.id && 'bg-brand-50']" @click="selected = s">
              <td class="font-semibold">
                {{ s.number }}
                <UBadge v-if="s.status === 'voided'" color="error" variant="soft" size="sm" class="ml-1">{{ t('sales.voidedBadge') }}</UBadge>
              </td>
              <td class="text-muted tabular-nums">{{ filter ? dateTime(s.created_at, lang) : time(s.created_at) }}</td>
              <td class="text-muted">{{ methodsOf(s) }}</td>
              <td :class="['text-right font-semibold tabular-nums', s.status === 'voided' && 'text-muted line-through']">{{ money(s.total_cents, currency) }}</td>
              <td><UBadge :color="syncBadge(s).color" variant="soft" :icon="syncBadge(s).icon" size="sm">{{ syncBadge(s).label }}</UBadge></td>
            </tr>
            <tr v-if="!sales.length">
              <td colspan="5" class="py-10 text-center text-muted">{{ t('sales.none') }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <aside class="flex min-h-0 flex-col gap-3 overflow-y-auto border-l border-line bg-surface p-4">
      <template v-if="selected">
        <div class="flex items-center justify-between">
          <div class="font-bold">{{ selected.number }}</div>
          <UBadge :color="syncBadge(selected).color" variant="soft" :icon="syncBadge(selected).icon">{{ syncBadge(selected).label }}</UBadge>
        </div>
        <UAlert v-if="selected.sync_state === 'rejected'" color="error" variant="soft" icon="i-lucide-triangle-alert" :description="t('sales.rejected', { error: selected.sync_error ?? '' })">
          <template #actions><UButton size="xs" color="error" @click="retry(selected)">{{ t('sales.retry') }}</UButton></template>
        </UAlert>
        <UAlert v-if="selected.void_sync === 'rejected'" color="error" variant="soft" :description="t('sales.voidRejected', { error: selected.void_error ?? '' })">
          <template #actions><UButton size="xs" color="error" @click="retry(selected)">{{ t('sales.retry') }}</UButton></template>
        </UAlert>
        <UAlert v-if="selected.void_sync === 'pending'" color="warning" variant="soft" icon="i-lucide-clock" :description="t('sales.voidPending')" />
        <UAlert
          v-if="selected.shortfall.length"
          color="warning"
          variant="soft"
          icon="i-lucide-package-x"
          :description="t('sales.shortfall', { items: selected.shortfall.map((x) => `${x.qty} × ${x.name}`).join(', ') })"
        />
        <ReceiptPreview :sale="selected" />
        <div class="flex gap-2">
          <UButton v-if="printerOn" class="flex-1" block color="neutral" variant="soft" icon="i-lucide-printer" @click="reprint(selected)">{{ t('sales.reprint') }}</UButton>
          <UButton
            v-if="selected.status === 'completed'"
            class="flex-1"
            block
            color="error"
            variant="soft"
            icon="i-lucide-ban"
            :disabled="!device?.cashier?.can_void"
            :title="device?.cashier?.can_void ? '' : t('sales.noVoidRight')"
            @click="voidOpen = true"
          >{{ t('sales.void') }}</UButton>
        </div>
      </template>
      <div v-else class="grid flex-1 place-items-center text-sm text-muted">
        <UIcon name="i-lucide-receipt-text" class="size-10" />
      </div>
    </aside>

    <UModal v-model:open="voidOpen" :title="t('sales.void')">
      <template #body>
        <form class="space-y-3" @submit.prevent="doVoid">
          <UFormField :label="t('sales.voidReason')"><UTextarea v-model="voidReason" class="w-full" autofocus :rows="2" maxlength="300" /></UFormField>
          <UButton type="submit" color="error" block :loading="busy" :disabled="!voidReason.trim()">{{ t('sales.voidConfirm') }}</UButton>
        </form>
      </template>
    </UModal>
  </div>
</template>
