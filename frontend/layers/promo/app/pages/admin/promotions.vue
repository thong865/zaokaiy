<script setup lang="ts">
import type { Campaign } from '../../utils/promo'
import { rewardText } from '../../utils/promo'

definePageMeta({ layout: 'admin', middleware: 'admin' })
const api = useApi()
const { t } = useI18n()
useHead({ title: () => t('promo.admin.title') })

const { data: list, refresh } = await useAsyncData('promo-admin', () => api<Campaign[]>('/admin/promo/campaigns'), { default: () => [] as Campaign[] })
interface Owed { shop_id: string; shop_name: string; currency: string; amount_cents: number; sales: number }
const { data: owed } = await useAsyncData('promo-liability', () => api<Owed[]>('/admin/promo/liability'), { default: () => [] as Owed[] })

const editing = ref<Campaign | 'new' | null>(null)
const busy = ref(false)
const error = ref('')
async function save(body: Record<string, unknown>) {
  busy.value = true
  error.value = ''
  try {
    if (editing.value === 'new') await api('/admin/promo/campaigns', { method: 'POST', body })
    else if (editing.value) await api(`/admin/promo/campaigns/${editing.value.id}`, { method: 'PATCH', body })
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
    await api(`/admin/promo/campaigns/${c.id}`, { method: 'PATCH', body: { ...c, active: !c.active } })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
const methods = (c: Campaign) => (c.providers.length ? c.providers.map((p) => t(`promo.method.${p}`)).join(', ') : t('promo.method.any'))
</script>

<template>
  <div>
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 class="page-title">{{ $t('promo.admin.title') }}</h1>
        <p class="mt-1 max-w-3xl text-sm text-muted">{{ $t('promo.admin.intro') }}</p>
      </div>
      <UButton icon="i-lucide-plus" @click="editing = 'new'">{{ $t('promo.shop.new') }}</UButton>
    </div>
    <p v-if="error && !editing" class="mt-3 text-sm text-brand-700">{{ error }}</p>

    <div v-if="list.length" class="card mt-5 overflow-x-auto">
      <table class="table">
        <thead>
          <tr>
            <th>{{ $t('promo.form.name') }}</th><th>{{ $t('promo.form.when') }}</th><th>{{ $t('promo.form.reward') }}</th>
            <th class="text-right">{{ $t('promo.stats.given') }}</th><th class="text-right">{{ $t('promo.stats.used') }}</th><th class="text-right">{{ $t('promo.stats.discount') }}</th><th />
          </tr>
        </thead>
        <tbody>
          <tr v-for="c in list" :key="c.id">
            <td><button class="text-left font-semibold hover:underline" @click="editing = c">{{ c.name }}</button><div class="text-xs text-muted">{{ fmtDate(c.starts_at) }}<template v-if="c.ends_at"> – {{ fmtDate(c.ends_at) }}</template></div></td>
            <td class="text-sm">{{ $t(`promo.trigger.${c.trigger}`) }}<div class="text-xs text-muted"><template v-if="c.trigger === 'signup'">{{ methods(c) }}</template><span v-else class="font-mono font-bold text-ink">{{ c.code }}</span></div></td>
            <td class="text-sm font-semibold text-brand-700">{{ rewardText(t, c) }}</td>
            <td class="text-right tabular-nums">{{ c.grants }}<span v-if="c.max_grants" class="text-muted"> / {{ c.max_grants }}</span></td>
            <td class="text-right tabular-nums">{{ c.used }}</td>
            <td class="text-right tabular-nums">{{ money(c.discount_cents, c.currency) }}</td>
            <td><USwitch :model-value="c.active" size="sm" :aria-label="$t('promo.form.active')" @update:model-value="toggle(c)" /></td>
          </tr>
        </tbody>
      </table>
    </div>
    <EmptyState v-else class="mt-5" icon="i-lucide-ticket-percent" :title="$t('promo.admin.emptyTitle')" :text="$t('promo.admin.emptyText')" />

    <h2 class="mt-8 font-bold">{{ $t('promo.admin.owedTitle') }}</h2>
    <p class="text-sm text-muted">{{ $t('promo.admin.owedIntro') }}</p>
    <div v-if="owed.length" class="card mt-3 overflow-x-auto">
      <table class="table">
        <thead><tr><th>{{ $t('common.shop') }}</th><th class="text-right">{{ $t('promo.admin.sales') }}</th><th class="text-right">{{ $t('promo.admin.owed') }}</th></tr></thead>
        <tbody>
          <tr v-for="o in owed" :key="`${o.shop_id}:${o.currency}`"><td class="font-semibold">{{ o.shop_name }}</td><td class="text-right tabular-nums">{{ o.sales }}</td><td class="text-right font-bold tabular-nums">{{ money(o.amount_cents, o.currency) }}</td></tr>
        </tbody>
      </table>
    </div>
    <p v-else class="mt-2 text-sm text-muted">{{ $t('promo.admin.owedNone') }}</p>

    <UModal :open="!!editing" :title="editing === 'new' ? $t('promo.shop.new') : $t('promo.shop.edit')" :ui="{ content: 'max-w-2xl' }" @update:open="(o: boolean) => !o && (editing = null)">
      <template #body>
        <p v-if="error" class="mb-3 text-sm text-brand-700">{{ error }}</p>
        <PromoCampaignForm v-if="editing" :key="editing === 'new' ? 'new' : editing.id" platform currency="LAK" :initial="editing === 'new' ? null : editing" :busy="busy" @save="save" @cancel="editing = null" />
      </template>
    </UModal>
  </div>
</template>
