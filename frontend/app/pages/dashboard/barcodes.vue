<script setup lang="ts">
import type { Product } from '~/utils/types'

/** Barcode labels: give products a barcode and print labels (label printer rolls or A4 sheets). */
definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const { t } = useI18n()
useHead({ title: () => t('barcode.title') + ' · zaokaiy' })
const api = useApi()
const { shopId, shop } = useShop()
const { print } = usePrint()
const cur = computed(() => shop.value?.currency ?? 'THB')

const q = ref('')
const onlyMissing = ref(false)
const { data: products, refresh } = await useAsyncData(
  'barcode-products',
  () => (shopId.value ? api<Product[]>(`/shops/${shopId.value}/products`, { query: { q: q.value || undefined } }) : Promise.resolve([] as Product[])),
  { watch: [shopId], default: () => [] as Product[] },
)
let timer: ReturnType<typeof setTimeout> | undefined
watch(q, () => { clearTimeout(timer); timer = setTimeout(refresh, 250) })

const rows = computed(() => products.value.filter((p) => p.status !== 'archived' && (!onlyMissing.value || !p.barcode)))
const missing = computed(() => products.value.filter((p) => p.status !== 'archived' && !p.barcode))
const copies = reactive<Record<string, number>>({})
const picked = computed(() => rows.value.filter((p) => (copies[p.id] ?? 0) > 0))
const totalLabels = computed(() => picked.value.reduce((n, p) => n + (copies[p.id] ?? 0), 0))
function toggle(p: Product) {
  copies[p.id] = copies[p.id] ? 0 : 1
}
function all(on: boolean) {
  for (const p of rows.value) copies[p.id] = on ? Math.max(copies[p.id] ?? 0, 1) : 0
}

const layout = ref<'roll40x30' | 'roll50x30' | 'a4'>('roll40x30')
const showPrice = ref(true)
const showName = ref(true)
onMounted(() => {
  try {
    const saved = JSON.parse(localStorage.getItem('zk_label_opts') || 'null')
    if (saved) {
      layout.value = saved.layout ?? layout.value
      showPrice.value = saved.showPrice ?? true
      showName.value = saved.showName ?? true
    }
  } catch {}
})
watch([layout, showPrice, showName], () => {
  try { localStorage.setItem('zk_label_opts', JSON.stringify({ layout: layout.value, showPrice: showPrice.value, showName: showName.value })) } catch {}
})

const msg = ref('')
const error = ref('')
const busy = ref(false)
async function generate(ids: string[]) {
  msg.value = error.value = ''
  busy.value = true
  try {
    const r = await api<{ assigned: { id: string; barcode: string }[] }>(`/shops/${shopId.value}/barcodes/generate`, { method: 'POST', body: { product_ids: ids } })
    msg.value = t('barcode.generated', { n: r.assigned.length }, r.assigned.length)
    await refresh()
    for (const a of r.assigned) copies[a.id] = Math.max(copies[a.id] ?? 0, 1)
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
const pickedWithoutCode = computed(() => picked.value.filter((p) => !p.barcode))
function printLabels() {
  const items = picked.value.filter((p) => p.barcode).map((p) => `${p.id}:${copies[p.id]}`).join(',')
  if (!items) return
  print(`/print/labels?items=${items}&layout=${layout.value}&price=${showPrice.value ? 1 : 0}&name=${showName.value ? 1 : 0}`)
}
</script>

<template>
  <div class="max-w-5xl">
    <h1 class="page-title">{{ $t('barcode.title') }}</h1>
    <p class="mt-1 max-w-2xl text-sm text-muted">{{ $t('barcode.intro') }}</p>

    <div v-if="missing.length" class="card mt-5 flex flex-wrap items-center justify-between gap-3 border-sun-400/60 bg-sun-400/10 p-4 text-sm">
      <span>{{ $t('barcode.missing', { n: missing.length }, missing.length) }}</span>
      <UButton color="neutral" size="sm" type="submit" :disabled="busy" @click="generate([])">{{ $t('barcode.generateAll') }}</UButton>
    </div>
    <p v-if="msg" class="mt-3 text-sm font-semibold text-mint-500">{{ msg }}</p>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>

    <div class="card mt-5 grid gap-4 p-4 sm:grid-cols-[1fr_auto] sm:items-end">
      <div class="grid gap-3 sm:grid-cols-3">
        <div>
          <label class="label">{{ $t('barcode.layout') }}</label>
          <select v-model="layout" class="input">
            <option value="roll40x30">{{ $t('barcode.layouts.roll40x30') }}</option>
            <option value="roll50x30">{{ $t('barcode.layouts.roll50x30') }}</option>
            <option value="a4">{{ $t('barcode.layouts.a4') }}</option>
          </select>
        </div>
        <label class="flex items-center gap-2 self-end pb-2 text-sm"><input v-model="showName" type="checkbox" class="size-4 accent-brand-500">{{ $t('barcode.showName') }}</label>
        <label class="flex items-center gap-2 self-end pb-2 text-sm"><input v-model="showPrice" type="checkbox" class="size-4 accent-brand-500">{{ $t('barcode.showPrice') }}</label>
      </div>
      <UButton type="submit" :disabled="!totalLabels || totalLabels === pickedWithoutCode.length" @click="printLabels">{{ $t('barcode.print', { n: totalLabels }, totalLabels) }}</UButton>
    </div>
    <p v-if="pickedWithoutCode.length" class="mt-2 text-xs text-amber-700">
      {{ $t('barcode.pickedWithoutCode', { n: pickedWithoutCode.length }, pickedWithoutCode.length) }}
      <button class="font-semibold underline" :disabled="busy" @click="generate(pickedWithoutCode.map((p) => p.id))">{{ $t('barcode.generateThese') }}</button>
    </p>

    <div class="mt-5 flex flex-wrap items-center gap-3">
      <UInput v-model="q" class="w-64" :placeholder="$t('barcode.search')" />
      <label class="flex items-center gap-2 text-sm"><input v-model="onlyMissing" type="checkbox" class="size-4 accent-brand-500">{{ $t('barcode.onlyMissing') }}</label>
      <div class="ml-auto flex gap-2 text-sm">
        <button class="font-semibold underline" @click="all(true)">{{ $t('barcode.selectAll') }}</button>
        <button class="text-muted underline" @click="all(false)">{{ $t('barcode.clear') }}</button>
      </div>
    </div>

    <div class="card mt-3 overflow-x-auto">
      <table class="table">
        <thead><tr><th class="w-10" /><th>{{ $t('common.product') }}</th><th>{{ $t('common.price') }}</th><th>{{ $t('barcode.code') }}</th><th class="w-28">{{ $t('barcode.copies') }}</th></tr></thead>
        <tbody>
          <tr v-for="p in rows" :key="p.id" :class="(copies[p.id] ?? 0) > 0 && 'bg-brand-50/40'">
            <td><input type="checkbox" class="size-4 accent-brand-500" :checked="(copies[p.id] ?? 0) > 0" :aria-label="p.name" @change="toggle(p)"></td>
            <td>
              <div class="flex items-center gap-3">
                <ProductThumb :image="p.cover" :src="p.images?.[0]" :name="p.name" sizes="40px" class="size-10 shrink-0" rounded="rounded-lg" />
                <div class="min-w-0"><div class="truncate font-semibold">{{ p.name }}</div><div class="font-mono text-xs text-muted">{{ p.sku }}</div></div>
              </div>
            </td>
            <td class="tabular-nums">{{ money(p.price_cents, cur) }}</td>
            <td>
              <BarcodeSvg v-if="p.barcode" :value="p.barcode" :height="28" class="h-10 w-36" />
              <button v-else class="text-xs font-semibold text-brand-600 underline" :disabled="busy" @click="generate([p.id])">{{ $t('barcode.generateOne') }}</button>
            </td>
            <td><UInput size="md" v-model.number="copies[p.id]" type="number" min="0" max="500" class="w-20" :placeholder="'0'" /></td>
          </tr>
        </tbody>
      </table>
      <p v-if="!rows.length" class="p-6 text-center text-sm text-muted">{{ $t('barcode.empty') }}</p>
    </div>
  </div>
</template>
