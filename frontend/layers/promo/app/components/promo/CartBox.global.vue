<script setup lang="ts">
import type { Applied } from '../../utils/promo'

/** Cart: coupon code + loyalty points per shop. Fills the `zk-promo:cart` state the cart page sends with the order. */
const props = defineProps<{ shops: { shop_id: string; name: string; currency: string; subtotal_cents: number; shipping_cents: number }[]; phone?: string }>()
const api = useApi()
const { loggedIn } = useAuth()
const hook = usePromoHookState('zk-promo:cart')
interface PointsInfo { shop_id: string; balance: number; point_value_cents: number; min_redeem_points: number; usable: boolean }
interface Quote { applied: Applied[]; points: PointsInfo[]; points_need_verified_phone: boolean }

const code = ref('')
const appliedCode = ref('')
const points = reactive<Record<string, number>>({})
const quote = ref<Quote | null>(null)
const error = ref('')
const busy = ref(false)
const shopsReady = computed(() => props.shops.length > 0 && props.shops.every((s) => s.shop_id))
const nameOf = (id: string) => props.shops.find((s) => s.shop_id === id)?.name ?? ''
const curOf = (id: string) => props.shops.find((s) => s.shop_id === id)?.currency ?? 'LAK'

function publish(q: Quote | null) {
  const pts = Object.fromEntries(Object.entries(points).filter(([, n]) => n > 0))
  hook.value = q && (appliedCode.value || Object.keys(pts).length)
    ? { body: { code: appliedCode.value || undefined, points: pts }, discount: Object.fromEntries(q.applied.map((a) => [a.shop_id ?? '', a.discount_cents])) }
    : { body: null, discount: {} }
}
async function run() {
  if (!loggedIn.value || !shopsReady.value) return
  busy.value = true
  error.value = ''
  try {
    quote.value = await api<Quote>('/promo/quote', {
      method: 'POST',
      body: {
        code: appliedCode.value || undefined,
        points: Object.fromEntries(Object.entries(points).filter(([, n]) => n > 0)),
        phone: props.phone || undefined,
        shops: props.shops.map((s) => ({ shop_id: s.shop_id, subtotal_cents: s.subtotal_cents, shipping_cents: s.shipping_cents })),
      },
    })
    publish(quote.value)
  } catch (e) {
    error.value = apiError(e)
    appliedCode.value = ''
    publish(null)
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
function setPoints(shop: string, n: number) {
  points[shop] = Math.max(0, Math.floor(n || 0))
  run()
}
const usedPoints = (shop: string) => quote.value?.applied.find((a) => a.shop_id === shop)?.points ?? 0
const couponLine = computed(() => quote.value?.applied.find((a) => a.coupon_cents > 0))

let timer: ReturnType<typeof setTimeout> | undefined
watch(() => JSON.stringify(props.shops), () => { clearTimeout(timer); timer = setTimeout(run, 400) })
onMounted(run)
onBeforeUnmount(() => (hook.value = { body: null, discount: {} }))
</script>

<template>
  <div class="space-y-2 border-t border-line pt-3">
    <div class="text-sm font-bold">{{ $t('promo.box.title') }}</div>
    <p v-if="!loggedIn" class="text-xs text-muted">{{ $t('promo.box.signIn') }}</p>
    <template v-else>
      <div v-if="!appliedCode" class="flex gap-2">
        <UInput v-model="code" class="flex-1 font-mono uppercase" :placeholder="$t('promo.box.codePh')" @keydown.enter.prevent="applyCode" />
        <UButton color="neutral" variant="soft" :disabled="!code.trim() || busy" @click="applyCode">{{ $t('promo.box.apply') }}</UButton>
      </div>
      <div v-else class="flex items-center justify-between rounded-xl bg-mint-500/10 px-3 py-2 text-sm">
        <span><span class="font-mono font-bold">{{ appliedCode }}</span><template v-if="couponLine"> · −{{ money(couponLine.coupon_cents, curOf(couponLine.shop_id ?? '')) }}<span v-if="shops.length > 1" class="text-xs text-muted"> ({{ nameOf(couponLine.shop_id ?? '') }})</span></template></span>
        <button type="button" class="text-xs font-semibold text-brand-700 underline" @click="removeCode">{{ $t('common.remove') }}</button>
      </div>
      <template v-for="p in quote?.points ?? []" :key="p.shop_id">
        <div v-if="p.usable && p.balance >= p.min_redeem_points" class="rounded-xl bg-[var(--field)] p-2.5 text-xs">
          <div>{{ $t('promo.box.youHave', { n: p.balance.toLocaleString(), shop: nameOf(p.shop_id), value: money(p.balance * p.point_value_cents, curOf(p.shop_id)) }) }}</div>
          <div class="mt-1.5 flex items-center gap-2">
            <UInput :model-value="points[p.shop_id] ? String(points[p.shop_id]) : ''" type="number" :min="p.min_redeem_points" :max="p.balance" size="sm" class="w-24" :placeholder="String(p.min_redeem_points)" @change="setPoints(p.shop_id, Number(($event.target as HTMLInputElement).value))" />
            <UButton size="xs" color="neutral" variant="soft" @click="setPoints(p.shop_id, p.balance)">{{ $t('promo.box.useMax') }}</UButton>
            <span v-if="usedPoints(p.shop_id)" class="font-semibold text-mint-500">−{{ money(usedPoints(p.shop_id) * p.point_value_cents, curOf(p.shop_id)) }}</span>
          </div>
        </div>
      </template>
      <p v-if="quote?.points_need_verified_phone" class="text-xs text-muted">{{ $t('promo.box.pointsNeedWhatsapp') }}</p>
      <p v-if="error" class="text-xs text-brand-700">{{ error }}</p>
    </template>
  </div>
</template>
