<script setup lang="ts">
import type { Campaign } from '../../utils/promo'
import { rewardText } from '../../utils/promo'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { t } = useI18n()
const { shopId, shop } = useShop()
useHead({ title: () => t('promo.shop.title') })

const { data: list, refresh } = await useAsyncData(
  'promo-campaigns',
  () => (shopId.value ? api<Campaign[]>(`/shops/${shopId.value}/promo/campaigns`) : Promise.resolve([])),
  { watch: [shopId], default: () => [] as Campaign[] },
)
const editing = ref<Campaign | 'new' | null>(null)
const busy = ref(false)
const error = ref('')
async function save(body: Record<string, unknown>) {
  busy.value = true
  error.value = ''
  try {
    if (editing.value === 'new') await api(`/shops/${shopId.value}/promo/campaigns`, { method: 'POST', body })
    else if (editing.value) await api(`/promo/campaigns/${editing.value.id}`, { method: 'PATCH', body })
    editing.value = null
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
async function toggle(c: Campaign) {
  try {
    await api(`/promo/campaigns/${c.id}`, { method: 'PATCH', body: { ...c, active: !c.active } })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
const live = (c: Campaign) => c.active && new Date(c.starts_at) <= new Date() && (!c.ends_at || new Date(c.ends_at) > new Date()) && (!c.max_grants || c.grants < c.max_grants)
</script>

<template>
  <div>
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 class="page-title">{{ $t('promo.shop.title') }}</h1>
        <p class="mt-1 max-w-3xl text-sm text-muted">{{ $t('promo.shop.intro') }}</p>
      </div>
      <div class="flex gap-2">
        <UButton color="neutral" variant="soft" to="/dashboard/loyalty" icon="i-lucide-gem">{{ $t('promo.nav.loyalty') }}</UButton>
        <UButton icon="i-lucide-plus" @click="editing = 'new'">{{ $t('promo.shop.new') }}</UButton>
      </div>
    </div>
    <p v-if="error && !editing" class="mt-3 text-sm text-brand-700">{{ error }}</p>

    <div v-if="list.length" class="mt-5 grid gap-3 md:grid-cols-2">
      <article v-for="c in list" :key="c.id" class="card p-4">
        <div class="flex items-start gap-3">
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-center gap-2">
              <h2 class="font-bold">{{ c.name }}</h2>
              <StatusBadge :status="live(c) ? 'active' : 'draft'" :label="live(c) ? $t('promo.status.live') : $t('promo.status.off')" />
            </div>
            <div class="mt-0.5 text-sm font-semibold text-brand-700">{{ rewardText(t, c) }}</div>
            <div class="mt-1 text-xs text-muted">
              {{ $t(`promo.trigger.${c.trigger}`) }}<template v-if="c.code"> · <span class="font-mono font-bold text-ink">{{ c.code }}</span></template>
              <template v-if="c.min_spend_cents"> · {{ $t('promo.minSpendShort', { amount: money(c.min_spend_cents, c.currency) }) }}</template>
              · {{ c.channels.map((ch) => $t(`promo.channel.${ch}`)).join(', ') }}
            </div>
          </div>
          <USwitch :model-value="c.active" size="sm" :aria-label="$t('promo.form.active')" @update:model-value="toggle(c)" />
        </div>
        <dl class="mt-3 grid grid-cols-3 gap-2 rounded-xl bg-[var(--field)] p-2.5 text-center text-xs">
          <div><dt class="text-muted">{{ $t('promo.stats.given') }}</dt><dd class="font-bold tabular-nums">{{ c.grants }}<span v-if="c.max_grants" class="font-normal text-muted"> / {{ c.max_grants }}</span></dd></div>
          <div><dt class="text-muted">{{ $t('promo.stats.used') }}</dt><dd class="font-bold tabular-nums">{{ c.used }}</dd></div>
          <div><dt class="text-muted">{{ $t('promo.stats.discount') }}</dt><dd class="font-bold tabular-nums">{{ money(c.discount_cents, c.currency) }}</dd></div>
        </dl>
        <div class="mt-2 flex items-center justify-between text-xs text-muted">
          <span>{{ fmtDate(c.starts_at) }}<template v-if="c.ends_at"> – {{ fmtDate(c.ends_at) }}</template></span>
          <button class="font-semibold text-brand-700 hover:underline" @click="editing = c">{{ $t('common.edit') }}</button>
        </div>
      </article>
    </div>
    <EmptyState v-else class="mt-5" icon="i-lucide-ticket-percent" :title="$t('promo.shop.emptyTitle')" :text="$t('promo.shop.emptyText')">
      <UButton @click="editing = 'new'">{{ $t('promo.shop.new') }}</UButton>
    </EmptyState>

    <UModal :open="!!editing" :title="editing === 'new' ? $t('promo.shop.new') : $t('promo.shop.edit')" :ui="{ content: 'max-w-2xl' }" @update:open="(o: boolean) => !o && (editing = null)">
      <template #body>
        <p v-if="error" class="mb-3 text-sm text-brand-700">{{ error }}</p>
        <PromoCampaignForm v-if="editing" :key="editing === 'new' ? 'new' : editing.id" :currency="shop?.currency ?? 'LAK'" :initial="editing === 'new' ? null : editing" :busy="busy" @save="save" @cancel="editing = null" />
      </template>
    </UModal>
  </div>
</template>
