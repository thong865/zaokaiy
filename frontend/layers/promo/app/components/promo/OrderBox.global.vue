<script setup lang="ts">
import type { Applied } from '../../utils/promo'

/** Chat-order checkout (/c/:token): coupon code + the member's points. Fills `zk-promo:order:<token>`. */
const props = defineProps<{ token: string; phone?: string }>()
const api = useApi()
const { loggedIn } = useAuth()
const hook = usePromoHookState(`zk-promo:order:${props.token}`)
interface Quote { applied: Applied; points_enabled: boolean; point_value_cents: number; min_redeem_points: number; points_trusted: boolean; current: { code: string | null; points: number } | null }

const code = ref('')
const appliedCode = ref('')
const points = ref(0)
const quote = ref<Quote | null>(null)
const error = ref('')
const busy = ref(false)
const cur = ref('LAK')
let restored = false

async function run() {
  busy.value = true
  error.value = ''
  try {
    quote.value = await api<Quote>(`/public/social-orders/${props.token}/promo`, {
      method: 'POST',
      body: { code: appliedCode.value || undefined, points: points.value || 0, phone: props.phone || undefined },
    })
    // Editing a confirmed order: start from the coupon/points it already uses.
    const c = quote.value.current
    if (!restored && c && !appliedCode.value && !points.value && (c.code || c.points)) {
      restored = true
      appliedCode.value = code.value = c.code ?? ''
      points.value = c.points
      return run()
    }
    restored = true
    const a = quote.value.applied
    hook.value = appliedCode.value || points.value
      ? { body: { code: appliedCode.value || undefined, points: points.value || 0 }, discount: { '': a.discount_cents } }
      : { body: null, discount: {} }
  } catch (e) {
    error.value = apiError(e)
    if (appliedCode.value) appliedCode.value = ''
    else points.value = 0
    hook.value = { body: null, discount: {} }
  } finally {
    busy.value = false
  }
}
function applyCode() {
  appliedCode.value = code.value.trim().toUpperCase()
  run()
}
function removeCode() {
  appliedCode.value = code.value = ''
  run()
}
const balance = computed(() => quote.value?.applied.points_balance ?? 0)
const canPoints = computed(() => !!quote.value?.points_enabled && balance.value >= (quote.value?.min_redeem_points ?? 1))
const route = useRoute()

let timer: ReturnType<typeof setTimeout> | undefined
watch(() => props.phone, () => { clearTimeout(timer); timer = setTimeout(run, 500) })
onMounted(async () => {
  try {
    cur.value = (await api<{ order: { currency: string } }>(`/public/social-orders/${props.token}`)).order.currency
  } catch { /* keep default */ }
  run()
})
</script>

<template>
  <div class="space-y-2 border-t border-line pt-3">
    <div class="font-bold">{{ $t('promo.box.title') }}</div>
    <div v-if="!appliedCode" class="flex gap-2">
      <UInput v-model="code" class="flex-1 font-mono uppercase" :placeholder="$t('promo.box.codePh')" @keydown.enter.prevent="applyCode" />
      <UButton color="neutral" variant="soft" :disabled="!code.trim() || busy" @click="applyCode">{{ $t('promo.box.apply') }}</UButton>
    </div>
    <div v-else class="flex items-center justify-between rounded-xl bg-mint-500/10 px-3 py-2 text-sm">
      <span><span class="font-mono font-bold">{{ appliedCode }}</span><template v-if="quote?.applied.coupon_cents"> · −{{ money(quote.applied.coupon_cents, cur) }}</template></span>
      <button type="button" class="text-xs font-semibold text-brand-700 underline" @click="removeCode">{{ $t('common.remove') }}</button>
    </div>
    <div v-if="canPoints" class="rounded-xl bg-[var(--field)] p-2.5 text-xs">
      <div>{{ $t('promo.box.youHaveHere', { n: balance.toLocaleString(), value: money(balance * (quote?.point_value_cents ?? 0), cur) }) }}</div>
      <div v-if="quote?.points_trusted" class="mt-1.5 flex items-center gap-2">
        <UInput :model-value="points ? String(points) : ''" type="number" :min="quote.min_redeem_points" :max="balance" size="sm" class="w-24" @change="points = Math.max(0, Math.floor(Number(($event.target as HTMLInputElement).value) || 0)); run()" />
        <UButton size="xs" color="neutral" variant="soft" @click="points = balance; run()">{{ $t('promo.box.useMax') }}</UButton>
        <span v-if="quote.applied.points_cents" class="font-semibold text-mint-500">−{{ money(quote.applied.points_cents, cur) }}</span>
      </div>
      <p v-else class="mt-1 text-muted">
        {{ $t('promo.box.signInToUse') }}
        <NuxtLink v-if="!loggedIn" :to="{ path: '/login', query: { next: route.fullPath } }" class="font-semibold text-brand-700 underline">{{ $t('promo.box.signInLink') }}</NuxtLink>
      </p>
    </div>
    <p v-if="error" class="text-xs text-brand-700">{{ error }}</p>
  </div>
</template>
