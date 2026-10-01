<script setup lang="ts">
/** Kitchen board: today's tickets by stage; refreshes every 15 seconds. */
import { nextStep, type FoodOrder } from '#restaurant/utils/restaurant'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId } = useShop()
const view = ref<'active' | 'done'>('active')
const { data: orders, refresh } = await useAsyncData(
  'kitchen',
  () => api<FoodOrder[]>(`/shops/${shopId.value}/restaurant/orders`, { query: { view: view.value } }),
  { watch: [shopId, view], default: () => [] },
)
const columns = computed(() => [
  { id: 'new', label: 'restaurant.board.new', list: orders.value.filter((o) => o.status === 'new') },
  { id: 'cooking', label: 'restaurant.board.cooking', list: orders.value.filter((o) => ['accepted', 'preparing'].includes(o.status)) },
  { id: 'ready', label: 'restaurant.board.ready', list: orders.value.filter((o) => o.status === 'ready') },
  { id: 'served', label: 'restaurant.board.toPay', list: orders.value.filter((o) => ['served', 'completed'].includes(o.status)) },
])
const busy = ref<string | null>(null)
const error = ref('')
async function update(o: FoodOrder, body: Record<string, unknown>) {
  busy.value = o.id
  error.value = ''
  try {
    await api(`/restaurant/orders/${o.id}`, { method: 'PATCH', body })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = null
  }
}
const confirmCancel = ref<string | null>(null)
let timer: ReturnType<typeof setInterval> | undefined
onMounted(() => (timer = setInterval(() => refresh(), 15_000)))
onBeforeUnmount(() => clearInterval(timer))
const { shop } = useShop()
</script>

<template>
  <div>
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 class="page-title">{{ $t('restaurant.board.title') }}</h1>
        <p class="text-sm text-muted">{{ $t('restaurant.board.subtitle') }}</p>
      </div>
      <div class="flex gap-2">
        <UButton v-for="v in (['active', 'done'] as const)" :key="v" size="md" :color="view === v ? 'primary' : 'neutral'" :variant="view === v ? 'solid' : 'soft'" @click="view = v">{{ $t(`restaurant.board.view.${v}`) }}</UButton>
        <UButton size="md" color="neutral" variant="soft" icon="i-lucide-refresh-cw" :aria-label="$t('restaurant.board.refresh')" @click="refresh()" />
      </div>
    </div>
    <p v-if="error" class="mt-3 text-sm text-brand-700" role="alert">{{ error }}</p>
    <EmptyState v-if="!orders.length" class="mt-6" :title="$t('restaurant.board.emptyTitle')" :text="$t('restaurant.board.emptyText')">
      <UButton v-if="shop" :to="`/s/${shop.slug}`" target="_blank" color="neutral" variant="soft">{{ $t('restaurant.board.openMenu') }}</UButton>
    </EmptyState>
    <div v-else class="mt-6 grid gap-4 md:grid-cols-2 xl:grid-cols-4">
      <section v-for="c in columns" :key="c.id" class="rounded-[20px] bg-[var(--field)] p-3" :aria-label="$t(c.label)">
        <h2 class="flex items-center justify-between px-1 pb-2 text-sm font-bold">{{ $t(c.label) }} <UBadge :label="String(c.list.length)" color="neutral" /></h2>
        <div class="space-y-3">
          <article v-for="o in c.list" :key="o.id" class="card p-4" :data-testid="`ticket-${o.number}`">
            <div class="flex items-start justify-between gap-2">
              <div>
                <div class="text-2xl font-extrabold tracking-tight">#{{ o.number }}</div>
                <div class="text-xs text-muted">{{ $t(`restaurant.mode.${o.mode}`) }}<template v-if="o.table_label"> · {{ o.table_label }}</template> · {{ fmtTime(o.created_at) }}</div>
              </div>
              <UBadge :color="o.paid ? 'success' : 'neutral'" :label="o.paid ? $t('restaurant.board.paid') : $t('restaurant.board.unpaid')" />
            </div>
            <ul class="mt-3 space-y-1 text-sm">
              <li v-for="l in o.items" :key="l.id"><b>{{ l.qty }}×</b> {{ l.name }}<span v-if="l.note" class="block pl-6 text-xs text-amber-700">↳ {{ l.note }}</span></li>
            </ul>
            <p v-if="o.note" class="mt-2 rounded-lg bg-sun-400/15 px-2 py-1 text-xs text-amber-800">{{ o.note }}</p>
            <p v-if="o.customer_name || o.customer_phone" class="mt-2 text-xs text-muted">{{ o.customer_name }} <a v-if="o.customer_phone" :href="telLink(o.customer_phone)" class="font-semibold text-brand-600">{{ o.customer_phone }}</a></p>
            <p v-if="o.address" class="text-xs text-muted">📍 {{ o.address }}</p>
            <div class="mt-3 flex items-center justify-between text-sm"><span class="text-muted">{{ $t('common.total') }}</span><b>{{ money(o.total_cents, o.currency) }}</b></div>
            <div v-if="!['completed', 'cancelled'].includes(o.status) || !o.paid" class="mt-3 flex flex-wrap gap-2">
              <UButton v-if="nextStep(o.status, o.mode)" size="sm" :loading="busy === o.id" @click="update(o, { status: nextStep(o.status, o.mode) })">{{ $t(`restaurant.board.to.${nextStep(o.status, o.mode)}`) }}</UButton>
              <UButton v-if="!o.paid && o.status !== 'cancelled'" size="sm" color="success" variant="soft" :loading="busy === o.id" @click="update(o, { paid: true, payment_method: 'cash' })">{{ $t('restaurant.board.markPaidCash') }}</UButton>
              <UButton v-if="!o.paid && o.status !== 'cancelled'" size="sm" color="neutral" variant="soft" @click="update(o, { paid: true, payment_method: 'transfer' })">{{ $t('restaurant.board.markPaidTransfer') }}</UButton>
              <template v-if="!['completed', 'cancelled'].includes(o.status)">
                <UButton v-if="confirmCancel !== o.id" size="sm" color="neutral" variant="ghost" @click="confirmCancel = o.id">{{ $t('common.cancel') }}</UButton>
                <UButton v-else size="sm" color="error" variant="soft" @click="update(o, { status: 'cancelled' }); confirmCancel = null">{{ $t('restaurant.board.confirmCancel') }}</UButton>
              </template>
            </div>
            <p v-if="o.status === 'cancelled'" class="mt-2 text-xs font-semibold text-brand-700">{{ $t('restaurant.status.cancelled') }}</p>
          </article>
        </div>
      </section>
    </div>
  </div>
</template>
