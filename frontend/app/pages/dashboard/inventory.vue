<script setup lang="ts">
import type { Product } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId } = useShop()
interface Movement {
  id: string
  product_id: string
  product_name: string
  delta: number
  stock_after: number
  reason: string
  ref_order_id: string | null
  note: string
  created_at: string
}
const { data: low, refresh: refreshLow } = await useAsyncData('low-stock', () => api<Product[]>(`/shops/${shopId.value}/inventory/low-stock`), { watch: [shopId], default: () => [] })
const { data: moves, refresh: refreshMoves } = await useAsyncData('movements', () => api<Movement[]>(`/shops/${shopId.value}/inventory/movements`, { query: { limit: 100 } }), { watch: [shopId], default: () => [] })

const restockQty = reactive<Record<string, number>>({})
const error = ref('')
async function restock(p: Product) {
  error.value = ''
  try {
    await api(`/products/${p.id}/stock`, { method: 'POST', body: { delta: Number(restockQty[p.id] || 10), reason: 'restock' } })
    await Promise.all([refreshLow(), refreshMoves()])
  } catch (e) {
    error.value = apiError(e)
  }
}
const reasonTone: Record<string, string> = { sale: 'text-brand-700', restock: 'text-mint-500', cancel: 'text-mint-500', return: 'text-mint-500', adjust: 'text-muted' }
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('dash.inventory.title') }}</h1>
    <p class="text-sm text-muted">{{ $t('dash.inventory.subtitle') }}</p>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>

    <h2 class="mt-8 font-bold">{{ $t('dash.inventory.lowStock') }} <span class="text-muted">({{ low.length }})</span></h2>
    <div v-if="low.length" class="mt-3 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      <div v-for="p in low" :key="p.id" class="card flex items-center gap-3 p-4">
        <ProductThumb :src="p.images?.[0]" :name="p.name" class="size-12 shrink-0" rounded="rounded-lg" />
        <div class="min-w-0 flex-1">
          <div class="truncate font-semibold">{{ p.name }}</div>
          <div class="text-xs" :class="p.stock === 0 ? 'font-bold text-brand-700' : 'text-muted'">{{ $t('dash.inventory.leftAlert', { n: p.stock, min: p.low_stock_threshold }) }}</div>
        </div>
        <input v-model.number="restockQty[p.id]" type="number" min="1" placeholder="10" class="input w-16 px-2 py-1.5" >
        <button class="btn-dark btn-sm" @click="restock(p)">{{ $t('dash.inventory.addStock') }}</button>
      </div>
    </div>
    <p v-else class="mt-3 text-sm text-muted">{{ $t('dash.inventory.allStocked') }}</p>

    <h2 class="mt-10 font-bold">{{ $t('dash.inventory.movements') }}</h2>
    <div v-if="moves.length" class="card mt-3 overflow-x-auto">
      <table class="table">
        <thead><tr><th>{{ $t('dash.inventory.when') }}</th><th>{{ $t('common.product') }}</th><th>{{ $t('dash.inventory.change') }}</th><th>{{ $t('dash.inventory.stockAfter') }}</th><th>{{ $t('dash.inventory.reason') }}</th><th>{{ $t('dash.inventory.ref') }}</th></tr></thead>
        <tbody>
          <tr v-for="m in moves" :key="m.id">
            <td class="whitespace-nowrap text-muted">{{ ago(m.created_at) }}</td>
            <td class="font-semibold">{{ m.product_name }}</td>
            <td class="font-bold" :class="m.delta > 0 ? 'text-mint-500' : 'text-brand-700'">{{ m.delta > 0 ? '+' : '' }}{{ m.delta }}</td>
            <td>{{ m.stock_after }}</td>
            <td><span class="capitalize" :class="reasonTone[m.reason]">{{ $te(`dash.inventory.reasons.${m.reason}`) ? $t(`dash.inventory.reasons.${m.reason}`) : m.reason }}</span></td>
            <td class="text-xs text-muted">{{ m.ref_order_id ? '#' + shortId(m.ref_order_id) : m.note }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <EmptyState v-else class="mt-3" :title="$t('dash.inventory.emptyMoves')" />
  </div>
</template>
