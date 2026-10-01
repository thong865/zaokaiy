<script setup lang="ts">
import type { VehicleLead } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { t } = useI18n()
const { shopId, shop } = useShop()

type Tab = '' | VehicleLead['status']
const tabs: Tab[] = ['new', 'contacted', 'scheduled', 'won', 'lost', '']
const status = ref<Tab>('')
const kind = ref('')
const q = ref('')
const search = ref('')
const { data, refresh } = await useAsyncData(
  'vehicle-leads',
  () => api<{ items: VehicleLead[]; counts: Record<string, number> }>(`/shops/${shopId.value}/leads`, {
    query: { status: status.value || undefined, kind: kind.value || undefined, q: search.value || undefined },
  }),
  { watch: [shopId, status, kind, search], default: () => ({ items: [] as VehicleLead[], counts: {} as Record<string, number> }) },
)
const total = computed(() => Object.values(data.value.counts).reduce((a, n) => a + n, 0))

const open = ref<string | null>(null)
const edit = reactive({ note: '', scheduled: '' })
function localInput(iso: string | null) {
  if (!iso) return ''
  const d = new Date(iso)
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`
}
function toggle(l: VehicleLead) {
  open.value = open.value === l.id ? null : l.id
  edit.note = l.seller_note
  edit.scheduled = localInput(l.scheduled_at ?? (l.kind === 'test_drive' ? l.preferred_at : null))
}
const busy = ref(false)
const error = ref('')
async function patch(l: VehicleLead, body: Record<string, unknown>) {
  busy.value = true
  error.value = ''
  try {
    await api(`/leads/${l.id}`, { method: 'PATCH', body })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
function saveDetails(l: VehicleLead) {
  const body: Record<string, unknown> = { seller_note: edit.note, scheduled_at: edit.scheduled ? new Date(edit.scheduled).toISOString() : null }
  if (edit.scheduled && (l.status === 'new' || l.status === 'contacted')) body.status = 'scheduled'
  return patch(l, body)
}
async function setVehicle(l: VehicleLead, sale_status: string) {
  busy.value = true
  error.value = ''
  try {
    await api(`/products/${l.product_id}/vehicle/status`, { method: 'POST', body: { sale_status } })
    if (sale_status === 'sold' && l.status !== 'won') await api(`/leads/${l.id}`, { method: 'PATCH', body: { status: 'won' } })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
const statusClass: Record<string, string> = {
  new: 'bg-brand-500 text-white',
  contacted: 'bg-sky-100 text-sky-800',
  scheduled: 'bg-sun-400/30 text-amber-800',
  won: 'bg-mint-500 text-white',
  lost: 'bg-line text-muted',
}
const kindIcon: Record<string, string> = { enquiry: '?', test_drive: '🚗', offer: '💰', reserve: '🔖', finance: '🏦' }
const waReply = (l: VehicleLead) =>
  waLink(l.phone, t('vehicle.leads.waReply', { name: l.name, shop: shop.value?.name ?? '', vehicle: l.product_name ?? '' }))
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('vehicle.leads.title') }}</h1>
    <p class="text-sm text-muted">{{ $t('vehicle.leads.subtitle') }}</p>

    <div class="mt-5 flex flex-wrap items-center gap-2">
      <button
        v-for="tb in tabs"
        :key="tb"
        class="chip border px-3 py-1.5"
        :class="status === tb ? 'border-brand-500 bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]' : 'border-transparent bg-surface shadow-card hover:text-brand-500'"
        @click="status = tb"
      >
        {{ tb ? $t(`vehicle.leads.status.${tb}`) : $t('common.all') }}
        <span class="opacity-60">{{ tb ? data.counts[tb] ?? 0 : total }}</span>
      </button>
      <select v-model="kind" class="input ml-auto w-auto py-1.5 text-sm" :aria-label="$t('vehicle.leads.kind')">
        <option value="">{{ $t('vehicle.leads.allKinds') }}</option>
        <option v-for="k in LEAD_KINDS" :key="k" :value="k">{{ $t(`vehicle.lead.kinds.${k}`) }}</option>
      </select>
      <form class="flex gap-2" @submit.prevent="search = q.trim()">
        <UInput size="md" v-model="q" class="w-56" :placeholder="$t('vehicle.leads.search')" :aria-label="$t('vehicle.leads.search')" />
      </form>
    </div>
    <p v-if="error" class="mt-3 text-sm text-brand-700" role="alert">{{ error }}</p>

    <div v-if="data.items.length" class="mt-5 space-y-3">
      <article v-for="l in data.items" :key="l.id" class="card overflow-hidden">
        <button class="flex w-full items-center gap-4 p-4 text-left hover:bg-paper/60" :aria-expanded="open === l.id" @click="toggle(l)">
          <ProductThumb :image="l.cover" :src="l.image" :name="l.product_name ?? ''" class="size-14 shrink-0" rounded="rounded-xl" />
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-center gap-2">
              <span class="chip" :class="statusClass[l.status]">{{ $t(`vehicle.leads.status.${l.status}`) }}</span>
              <span class="text-sm font-bold">{{ kindIcon[l.kind] }} {{ $t(`vehicle.lead.kinds.${l.kind}`) }}</span>
              <span v-if="l.deposit_paid_at" class="chip bg-mint-500/15 text-mint-500">{{ $t('vehicle.leads.depositPaid') }}</span>
              <span v-if="l.sale_status && l.sale_status !== 'available'" class="chip bg-night/80 text-white">{{ optLabel('sale', l.sale_status) }}</span>
            </div>
            <div class="mt-1 truncate text-sm"><b>{{ l.name }}</b> · {{ l.phone }} · <span class="text-muted">{{ l.product_name }}</span></div>
            <div class="mt-0.5 text-xs text-muted">
              <span v-if="l.offer_cents">{{ $t('vehicle.leads.offer', { amount: money(l.offer_cents, l.currency), price: money(l.price_cents, l.currency) }) }} · </span>
              <span v-if="l.scheduled_at">{{ $t('vehicle.leads.scheduledFor', { date: fmtWeekdayTime(l.scheduled_at) }) }} · </span>
              <span v-else-if="l.preferred_at">{{ $t('vehicle.leads.wants', { date: fmtWeekdayTime(l.preferred_at) }) }} · </span>
              <span v-if="l.kind === 'reserve' && l.deposit_cents">{{ $t('vehicle.leads.deposit', { amount: money(l.deposit_cents, l.currency) }) }} · </span>
              {{ $t('vehicle.leads.received', { ago: ago(l.created_at) }) }}
            </div>
          </div>
          <span class="text-muted">{{ open === l.id ? '▴' : '▾' }}</span>
        </button>
        <div v-if="open === l.id" class="space-y-4 border-t border-line p-4">
          <p v-if="l.message" class="whitespace-pre-line rounded-xl bg-paper p-3 text-sm">“{{ l.message }}”</p>
          <div class="flex flex-wrap gap-2">
            <UButton color="neutral" size="sm" v-if="telLink(l.phone)" :to="telLink(l.phone)">📞 {{ $t('vehicle.detail.call') }}</UButton>
            <UButton color="neutral" variant="soft" size="sm" v-if="waReply(l)" :to="waReply(l)" target="_blank" rel="noopener">💬 {{ $t('vehicle.detail.whatsapp') }}</UButton>
            <UButton color="neutral" variant="soft" size="sm" :to="`/p/${l.product_id}`" target="_blank">{{ $t('vehicle.leads.openListing') }}</UButton>
            <UButton color="neutral" variant="soft" size="sm" :to="`/dashboard/products/${l.product_id}`">{{ $t('common.edit') }}</UButton>
          </div>
          <div>
            <div class="label">{{ $t('common.statusLabel') }}</div>
            <div class="flex flex-wrap gap-1.5">
              <button
                v-for="s in LEAD_STATUSES"
                :key="s"
                class="chip border px-3 py-1.5"
                :class="l.status === s ? 'border-brand-500 bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]' : 'border-transparent bg-surface shadow-card hover:text-brand-500'"
                :disabled="busy"
                @click="patch(l, { status: s })"
              >
                {{ $t(`vehicle.leads.status.${s}`) }}
              </button>
            </div>
          </div>
          <form class="grid gap-3 sm:grid-cols-[220px_1fr_auto] sm:items-end" @submit.prevent="saveDetails(l)">
            <div><label :for="`sch-${l.id}`" class="label">{{ $t('vehicle.leads.schedule') }}</label><UInput :id="`sch-${l.id}`" v-model="edit.scheduled" type="datetime-local" /></div>
            <div><label :for="`note-${l.id}`" class="label">{{ $t('vehicle.leads.note') }}</label><UInput :id="`note-${l.id}`" v-model="edit.note" maxlength="2000" :placeholder="$t('vehicle.leads.notePh')" /></div>
            <UButton size="sm" type="submit" :disabled="busy">{{ $t('common.save') }}</UButton>
          </form>
          <div class="flex flex-wrap items-center gap-2 rounded-xl bg-paper p-3 text-sm">
            <span class="font-semibold">{{ $t('vehicle.leads.vehicle') }}:</span>
            <span>{{ optLabel('sale', l.sale_status) }}</span>
            <span class="flex-1" />
            <template v-if="l.kind === 'reserve' || l.deposit_paid_at">
              <UButton color="neutral" size="sm" type="submit" v-if="!l.deposit_paid_at" :disabled="busy || l.sale_status === 'sold'" @click="patch(l, { deposit_paid: true, status: l.status === 'new' ? 'contacted' : undefined })">{{ $t('vehicle.leads.markDeposit') }}</UButton>
              <UButton color="neutral" variant="soft" size="sm" type="submit" v-else :disabled="busy" @click="patch(l, { deposit_paid: false })">{{ $t('vehicle.leads.undoDeposit') }}</UButton>
            </template>
            <UButton color="neutral" variant="soft" size="sm" type="submit" v-if="l.sale_status !== 'sold'" :disabled="busy" @click="setVehicle(l, 'sold')">{{ $t('vehicle.leads.markSold') }}</UButton>
            <UButton color="neutral" variant="soft" size="sm" type="submit" v-else :disabled="busy" @click="setVehicle(l, 'available')">{{ $t('vehicle.leads.markAvailable') }}</UButton>
          </div>
        </div>
      </article>
    </div>
    <EmptyState v-else class="mt-6" :title="$t('vehicle.leads.empty')" :text="$t('vehicle.leads.emptyHint')" />
  </div>
</template>
