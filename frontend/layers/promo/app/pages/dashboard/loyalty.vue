<script setup lang="ts">
import type { LedgerRow, LoyaltyMember, LoyaltySettings, PromoChannel } from '../../utils/promo'
import { PROMO_CHANNELS } from '../../utils/promo'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { t } = useI18n()
const { shopId, shop } = useShop()
useHead({ title: () => t('promo.loyalty.title') })
const cur = computed(() => shop.value?.currency ?? 'LAK')

const { data: settings, refresh: refreshSettings } = await useAsyncData(
  'loyalty-settings',
  () => (shopId.value ? api<LoyaltySettings>(`/shops/${shopId.value}/loyalty`) : Promise.resolve(null)),
  { watch: [shopId] },
)
// The form works in major units: "spend ₭10,000 → 1 point", "1 point = ₭100".
const f = reactive({ enabled: false, earn: '', value: '', min: 10, maxPct: 50, expiry: 0, channels: [...PROMO_CHANNELS] as PromoChannel[] })
watch(settings, (s) => {
  if (!s) return
  Object.assign(f, { enabled: s.enabled, earn: String(s.earn_per_cents / 100), value: String(s.point_value_cents / 100), min: s.min_redeem_points, maxPct: s.max_redeem_bps / 100, expiry: s.expiry_days, channels: [...s.channels] })
}, { immediate: true })
const toCents = (v: string | number) => Math.round((Number(v) || 0) * 100)
const example = computed(() => {
  const earn = toCents(f.earn), val = toCents(f.value)
  if (!earn || !val) return ''
  const spend = earn * 100
  return t('promo.loyalty.example', { spend: money(spend, cur.value), points: 100, value: money(100 * val, cur.value), pct: Math.round((val / earn) * 10000) / 100 })
})
const busy = ref(false)
const msg = ref('')
const error = ref('')
async function saveSettings() {
  busy.value = true
  error.value = msg.value = ''
  try {
    settings.value = await api<LoyaltySettings>(`/shops/${shopId.value}/loyalty`, {
      method: 'PUT',
      body: { enabled: f.enabled, earn_per_cents: toCents(f.earn), point_value_cents: toCents(f.value), min_redeem_points: Number(f.min) || 1, max_redeem_bps: Math.round((Number(f.maxPct) || 0) * 100), expiry_days: Number(f.expiry) || 0, channels: f.channels },
    })
    msg.value = t('common.saved')
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
function toggleChannel(ch: PromoChannel) {
  const i = f.channels.indexOf(ch)
  if (i >= 0) f.channels.splice(i, 1)
  else f.channels.push(ch)
}

// Members
const q = ref('')
const { data: members, refresh: refreshMembers } = await useAsyncData(
  'loyalty-members',
  () => (shopId.value ? api<LoyaltyMember[]>(`/shops/${shopId.value}/loyalty/members`, { query: { q: q.value || undefined } }) : Promise.resolve([])),
  { watch: [shopId], default: () => [] as LoyaltyMember[] },
)
let timer: ReturnType<typeof setTimeout> | undefined
watch(q, () => { clearTimeout(timer); timer = setTimeout(() => refreshMembers(), 300) })

const open = ref<{ member: LoyaltyMember; ledger: LedgerRow[] } | null>(null)
const adj = reactive({ delta: '', note: '', error: '' })
async function openMember(m: LoyaltyMember) {
  Object.assign(adj, { delta: '', note: '', error: '' })
  open.value = await api(`/shops/${shopId.value}/loyalty/members/${m.id}`)
}
async function adjust() {
  adj.error = ''
  try {
    open.value = await api(`/shops/${shopId.value}/loyalty/members/${open.value!.member.id}/adjust`, { method: 'POST', body: { delta: Math.round(Number(adj.delta) || 0), note: adj.note } })
    Object.assign(adj, { delta: '', note: '' })
    await Promise.all([refreshMembers(), refreshSettings()])
  } catch (e) {
    adj.error = apiError(e)
  }
}
const reasonColor: Record<string, string> = { earn: 'text-mint-500', bonus: 'text-mint-500', redeem: 'text-brand-700', expire: 'text-muted', reverse: 'text-amber-800', adjust: 'text-ink' }
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('promo.loyalty.title') }}</h1>
    <p class="mt-1 max-w-3xl text-sm text-muted">{{ $t('promo.loyalty.intro') }}</p>

    <div class="mt-5 grid gap-6 lg:grid-cols-[380px_1fr]">
      <form class="card h-fit space-y-4 p-5" @submit.prevent="saveSettings">
        <label class="flex items-center gap-2 font-bold"><USwitch v-model="f.enabled" /> {{ $t('promo.loyalty.enabled') }}</label>
        <UFormField :label="$t('promo.loyalty.earn', { cur })" :hint="$t('promo.loyalty.earnHint')">
          <UInput v-model="f.earn" type="number" min="0.01" step="0.01" class="w-full" />
        </UFormField>
        <UFormField :label="$t('promo.loyalty.value', { cur })">
          <UInput v-model="f.value" type="number" min="0.01" step="0.01" class="w-full" />
        </UFormField>
        <p v-if="example" class="rounded-xl bg-[var(--field)] p-2.5 text-xs">{{ example }}</p>
        <div class="grid grid-cols-2 gap-3">
          <UFormField :label="$t('promo.loyalty.min')"><UInput v-model.number="f.min" type="number" min="1" class="w-full" /></UFormField>
          <UFormField :label="$t('promo.loyalty.maxPct')"><UInput v-model.number="f.maxPct" type="number" min="1" max="100" class="w-full" /></UFormField>
        </div>
        <UFormField :label="$t('promo.loyalty.expiry')" :hint="$t('promo.loyalty.expiryHint')">
          <UInput v-model.number="f.expiry" type="number" min="0" max="3650" class="w-full" />
        </UFormField>
        <div>
          <div class="text-sm font-semibold">{{ $t('promo.form.channels') }}</div>
          <div class="mt-2 flex flex-wrap gap-2">
            <button v-for="ch in PROMO_CHANNELS" :key="ch" type="button" class="rounded-full border px-3 py-1 text-sm font-semibold" :class="f.channels.includes(ch) ? 'border-ink bg-ink text-paper' : 'border-line'" @click="toggleChannel(ch)">{{ $t(`promo.channel.${ch}`) }}</button>
          </div>
        </div>
        <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
        <p v-if="msg" class="text-sm text-mint-500">{{ msg }}</p>
        <UButton type="submit" class="w-full" :loading="busy">{{ $t('common.save') }}</UButton>
      </form>

      <section>
        <div class="grid grid-cols-3 gap-3">
          <StatCard :label="$t('promo.loyalty.members')" :value="String(settings?.stats?.members ?? 0)" />
          <StatCard :label="$t('promo.loyalty.outstanding')" :value="(settings?.stats?.outstanding_points ?? 0).toLocaleString()" />
          <StatCard :label="$t('promo.loyalty.outstandingValue')" :value="money(settings?.stats?.outstanding_cents ?? 0, cur)" />
        </div>
        <UInput v-model="q" class="mt-4 w-full" icon="i-lucide-search" :placeholder="$t('promo.loyalty.search')" />
        <div v-if="members.length" class="card mt-3 overflow-x-auto">
          <table class="table">
            <thead><tr><th>{{ $t('promo.loyalty.member') }}</th><th class="text-right">{{ $t('promo.loyalty.balance') }}</th><th class="text-right">{{ $t('promo.loyalty.orders') }}</th><th>{{ $t('promo.loyalty.lastActivity') }}</th></tr></thead>
            <tbody>
              <tr v-for="m in members" :key="m.id" class="cursor-pointer hover:bg-[var(--field)]" @click="openMember(m)">
                <td><div class="font-semibold">{{ m.name || '—' }}</div><div class="font-mono text-xs text-muted">{{ m.phone }}</div></td>
                <td class="text-right font-bold tabular-nums">{{ m.balance.toLocaleString() }}</td>
                <td class="text-right tabular-nums">{{ m.orders }}</td>
                <td class="text-xs text-muted">{{ ago(m.last_activity_at) }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <EmptyState v-else class="mt-3" icon="i-lucide-gem" :title="$t('promo.loyalty.noMembers')" :text="$t('promo.loyalty.noMembersText')" />
      </section>
    </div>

    <UModal :open="!!open" :title="open ? (open.member.name || open.member.phone) : ''" @update:open="(o: boolean) => !o && (open = null)">
      <template #body>
        <div v-if="open" class="space-y-4">
          <div class="flex items-center justify-between rounded-xl bg-[var(--field)] p-3">
            <div class="font-mono text-sm">{{ open.member.phone }}</div>
            <div class="text-right"><div class="text-2xl font-extrabold tabular-nums">{{ open.member.balance.toLocaleString() }}</div><div class="text-xs text-muted">{{ $t('promo.loyalty.points') }}</div></div>
          </div>
          <form class="flex flex-wrap gap-2" @submit.prevent="adjust">
            <UInput v-model="adj.delta" type="number" class="w-28" :placeholder="$t('promo.loyalty.adjustPh')" />
            <UInput v-model="adj.note" class="min-w-40 flex-1" :placeholder="$t('promo.loyalty.adjustNote')" />
            <UButton type="submit" color="neutral">{{ $t('promo.loyalty.adjust') }}</UButton>
          </form>
          <p v-if="adj.error" class="text-sm text-brand-700">{{ adj.error }}</p>
          <ul class="max-h-80 divide-y divide-line/70 overflow-y-auto text-sm">
            <li v-for="l in open.ledger" :key="l.id" class="flex items-center gap-3 py-2">
              <div class="min-w-0 flex-1">
                <div class="font-semibold">{{ $t(`promo.reason.${l.reason}`) }}<span v-if="l.ref_type" class="font-normal text-muted"> · {{ $t(`promo.channel.${l.ref_type === 'order' ? 'web' : l.ref_type}`) }}</span></div>
                <div class="text-xs text-muted">{{ fmtDateTime(l.created_at) }}<template v-if="l.note && !l.note.startsWith('campaign ')"> · {{ l.note }}</template></div>
              </div>
              <div class="text-right tabular-nums"><div class="font-bold" :class="reasonColor[l.reason]">{{ l.delta > 0 ? '+' : '' }}{{ l.delta }}</div><div class="text-xs text-muted">{{ l.balance_after }}</div></div>
            </li>
          </ul>
        </div>
      </template>
    </UModal>
  </div>
</template>
