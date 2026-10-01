<script setup lang="ts">
import type { Applied, CouponView, LoyaltyMember, LoyaltySettings } from '../../utils/promo'
import { rewardText } from '../../utils/promo'

/** POS: look up the customer's phone → their points and coupons; take them off the bill (`zk-promo:pos`). */
const props = defineProps<{ shopId: string; subtotal: number; currency: string }>()
const api = useApi()
const { t } = useI18n()
const hook = usePromoHookState('zk-promo:pos')
interface Lookup { phone: string; member: LoyaltyMember | null; settings: LoyaltySettings; coupons: CouponView[] }

const phone = ref('')
const info = ref<Lookup | null>(null)
const code = ref('')
const points = ref(0)
const applied = ref<Applied | null>(null)
const error = ref('')
const busy = ref(false)

async function lookup() {
  if (!phone.value.trim()) return
  busy.value = true
  error.value = ''
  try {
    info.value = await api<Lookup>(`/shops/${props.shopId}/loyalty/lookup`, { query: { phone: phone.value } })
    code.value = ''
    points.value = 0
    await run()
  } catch (e) {
    error.value = apiError(e)
    info.value = null
  } finally {
    busy.value = false
  }
}
async function run() {
  if (!info.value) return
  error.value = ''
  try {
    applied.value = await api<Applied>(`/shops/${props.shopId}/loyalty/quote`, {
      method: 'POST',
      body: { member_phone: info.value.phone, code: code.value || undefined, points: points.value || 0, subtotal_cents: props.subtotal },
    })
    hook.value = { body: { member_phone: info.value.phone, code: code.value || undefined, points: points.value || 0 }, discount: { '': applied.value.discount_cents } }
  } catch (e) {
    error.value = apiError(e)
    code.value = ''
    points.value = 0
    applied.value = null
    hook.value = { body: { member_phone: info.value.phone }, discount: {} }
  }
}
function useCoupon(c: string) {
  code.value = code.value === c ? '' : c
  run()
}
function clear() {
  phone.value = code.value = ''
  points.value = 0
  info.value = applied.value = null
  error.value = ''
  hook.value = { body: null, discount: {} }
}
// The till clears the state after a sale / new bill.
watch(() => hook.value.body, (b) => { if (!b && info.value) { phone.value = code.value = ''; points.value = 0; info.value = applied.value = null } })
let timer: ReturnType<typeof setTimeout> | undefined
watch(() => props.subtotal, () => { if (info.value && (code.value || points.value)) { clearTimeout(timer); timer = setTimeout(run, 300) } })
const s = computed(() => info.value?.settings)
const canPoints = computed(() => !!s.value?.enabled && s.value.channels.includes('pos') && (info.value?.member?.balance ?? 0) >= (s.value?.min_redeem_points ?? 1))
</script>

<template>
  <div class="space-y-2 text-sm">
    <form v-if="!info" class="flex gap-2" @submit.prevent="lookup">
      <UInput v-model="phone" class="flex-1" inputmode="tel" icon="i-lucide-gem" :placeholder="$t('promo.pos.phonePh')" />
      <UButton type="submit" color="neutral" variant="soft" size="sm" :loading="busy">{{ $t('promo.pos.lookup') }}</UButton>
    </form>
    <div v-else class="space-y-2 rounded-xl bg-[var(--field)] p-2.5">
      <div class="flex items-center justify-between gap-2">
        <div class="min-w-0">
          <div class="truncate font-semibold">{{ info.member?.name || info.phone }}</div>
          <div class="text-xs text-muted">{{ info.member ? $t('promo.pos.balance', { n: info.member.balance.toLocaleString() }) : $t('promo.pos.newMember') }}</div>
        </div>
        <button type="button" class="text-xs font-semibold text-brand-700 underline" @click="clear">{{ $t('common.remove') }}</button>
      </div>
      <div v-if="canPoints" class="flex items-center gap-2">
        <UInput :model-value="points ? String(points) : ''" type="number" :min="s!.min_redeem_points" :max="info.member!.balance" size="sm" class="w-24" :placeholder="$t('promo.pos.points')" @change="points = Math.max(0, Math.floor(Number(($event.target as HTMLInputElement).value) || 0)); run()" />
        <UButton size="xs" color="neutral" variant="soft" @click="points = info.member!.balance; run()">{{ $t('promo.box.useMax') }}</UButton>
        <span v-if="applied?.points_cents" class="text-xs font-semibold text-mint-500">−{{ money(applied.points_cents, currency) }}</span>
      </div>
      <div v-if="info.coupons.length" class="flex flex-wrap gap-1.5">
        <button v-for="c in info.coupons" :key="c.id" type="button" class="rounded-lg border px-2 py-1 text-left text-xs" :class="code === c.code ? 'border-mint-500 bg-mint-500/10' : 'border-line bg-surface'" @click="useCoupon(c.code)">
          <span class="font-semibold">{{ rewardText(t, c) }}</span> <span class="font-mono text-muted">{{ c.code }}</span>
        </button>
      </div>
      <form v-else class="flex gap-2" @submit.prevent="run">
        <UInput v-model="code" size="sm" class="flex-1 font-mono uppercase" :placeholder="$t('promo.box.codePh')" />
        <UButton type="submit" size="xs" color="neutral" variant="soft">{{ $t('promo.box.apply') }}</UButton>
      </form>
      <p class="text-xs text-muted">{{ $t('promo.pos.earnHint') }}</p>
    </div>
    <p v-if="error" class="text-xs text-brand-700">{{ error }}</p>
  </div>
</template>
