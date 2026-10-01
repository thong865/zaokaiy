<script setup lang="ts">
/**
 * Storefront of a restaurant (/s/<slug>): menu by section, basket, and ordering —
 * dine-in after scanning the table QR (?table=<token>), takeaway or delivery.
 */
import type { Shop } from '~/utils/types'
import { localName, type MenuItem, type MenuSection, type OrderMode, type RestaurantSettings } from '#restaurant/utils/restaurant'

const props = defineProps<{ shop: Shop }>()
const api = useApi()
const route = useRoute()
const { locale, t } = useI18n()

const { data } = await useAsyncData(`menu:${props.shop.slug}`, () =>
  api<{ shop: Shop & { phone: string; address: string }; settings: RestaurantSettings; sections: MenuSection[]; items: MenuItem[] }>(
    `/restaurant/menu/${props.shop.slug}`,
  ),
)
const tableToken = computed(() => (route.query.table as string) || '')
const { data: table } = await useAsyncData(`table:${tableToken.value}`, () =>
  tableToken.value ? api<{ label: string; shop_slug: string }>(`/restaurant/tables/${tableToken.value}`).catch(() => null) : Promise.resolve(null),
)

const settings = computed(() => data.value?.settings)
const modes = computed<OrderMode[]>(() => {
  const s = settings.value
  if (!s) return []
  return (['dine_in', 'takeaway', 'delivery'] as OrderMode[]).filter((m) => s[m] && (m !== 'dine_in' || table.value))
})
const mode = ref<OrderMode>(table.value ? 'dine_in' : 'takeaway')
watchEffect(() => {
  if (modes.value.length && !modes.value.includes(mode.value)) mode.value = modes.value[0]!
})

const groups = computed(() => {
  const d = data.value
  if (!d) return []
  const out = d.sections.map((s) => ({ id: s.id, name: localName(s, locale.value), items: d.items.filter((i) => i.section_id === s.id) }))
  const loose = d.items.filter((i) => !i.section_id || !d.sections.some((s) => s.id === i.section_id))
  if (loose.length) out.push({ id: 'other', name: t('restaurant.menu.other'), items: loose })
  return out.filter((g) => g.items.length)
})

// Basket: item id → qty (+ note)
const basket = reactive<Record<string, { qty: number; note: string }>>({})
const lines = computed(() =>
  Object.entries(basket)
    .filter(([, l]) => l.qty > 0)
    .map(([id, l]) => ({ item: data.value!.items.find((i) => i.id === id)!, ...l }))
    .filter((l) => l.item),
)
const count = computed(() => lines.value.reduce((n, l) => n + l.qty, 0))
const subtotal = computed(() => lines.value.reduce((n, l) => n + l.qty * l.item.price_cents, 0))
const service = computed(() => Math.round((subtotal.value * (settings.value?.service_charge_bps ?? 0)) / 10000))
function add(i: MenuItem, d = 1) {
  const cur = basket[i.id]?.qty ?? 0
  basket[i.id] = { qty: Math.max(0, Math.min(99, cur + d)), note: basket[i.id]?.note ?? '' }
}

const checkout = ref(false)
const form = reactive({ customer_name: '', customer_phone: '', address: '', note: '' })
const error = ref('')
const sending = ref(false)
async function place() {
  error.value = ''
  sending.value = true
  try {
    const o = await api<{ track_token: string }>(`/restaurant/menu/${props.shop.slug}/orders`, {
      method: 'POST',
      body: {
        mode: mode.value,
        table_token: mode.value === 'dine_in' ? tableToken.value : undefined,
        items: lines.value.map((l) => ({ item_id: l.item.id, qty: l.qty, note: l.note })),
        ...form,
      },
    })
    await navigateTo(`/r/o/${o.track_token}`)
  } catch (e) {
    error.value = apiError(e)
  } finally {
    sending.value = false
  }
}
const cur = computed(() => data.value?.shop.currency ?? props.shop.currency)
const spicy = (n: number) => '🌶'.repeat(n)
</script>

<template>
  <div v-if="data" class="mx-auto max-w-5xl px-4 pb-32 pt-8">
    <div class="card flex flex-wrap items-center gap-5 p-6">
      <ProductThumb :src="shop.logo_url" :name="shop.name" class="size-20" rounded="rounded-3xl" />
      <div class="min-w-0 flex-1">
        <div class="flex flex-wrap items-center gap-2">
          <h1 class="text-2xl font-extrabold tracking-tight">{{ shop.name }}</h1>
          <span class="chip bg-amber-500/15 text-amber-700">{{ $t('common.verticals.restaurant') }}</span>
          <KybVerifiedBadge v-if="shop.kyb_verified_at" />
        </div>
        <p class="mt-1 text-sm text-muted">{{ shop.description }}</p>
        <p class="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted">
          <span v-if="data.shop.address">📍 {{ data.shop.address }}</span>
          <span>⏱ {{ $t('restaurant.menu.prep', { n: settings?.prep_minutes }) }}</span>
          <a v-if="telLink(data.shop.phone)" :href="telLink(data.shop.phone)" class="font-semibold text-brand-600">📞 {{ data.shop.phone }}</a>
        </p>
      </div>
      <UBadge v-if="table" color="primary" variant="solid" size="lg" icon="i-lucide-armchair">{{ $t('restaurant.menu.table', { label: table.label }) }}</UBadge>
    </div>

    <div v-if="!settings?.accepting_orders" class="mt-4 rounded-2xl bg-sun-400/20 p-4 text-sm font-semibold text-amber-800" role="status">{{ $t('restaurant.menu.closed') }}</div>

    <nav v-if="groups.length > 1" class="sticky top-16 z-10 -mx-4 mt-6 flex gap-2 overflow-x-auto bg-paper/90 px-4 py-2 backdrop-blur">
      <a v-for="g in groups" :key="g.id" :href="`#sec-${g.id}`" class="shrink-0 rounded-xl bg-surface px-3.5 py-1.5 text-sm font-semibold shadow-card hover:text-brand-500">{{ g.name }}</a>
    </nav>

    <EmptyState v-if="!groups.length" class="mt-6" :title="$t('restaurant.menu.emptyTitle')" :text="$t('restaurant.menu.emptyText')" />
    <section v-for="g in groups" :id="`sec-${g.id}`" :key="g.id" class="mt-8 scroll-mt-32">
      <h2 class="text-xl font-extrabold tracking-tight">{{ g.name }}</h2>
      <div class="mt-3 grid gap-3 sm:grid-cols-2">
        <article v-for="i in g.items" :key="i.id" class="card flex gap-3 p-3" :class="!i.available && 'opacity-60'" :data-testid="`dish-${i.name}`">
          <ProductThumb :src="i.image_url || null" :name="i.name" class="size-24 shrink-0" rounded="rounded-2xl" />
          <div class="flex min-w-0 flex-1 flex-col">
            <h3 class="font-bold leading-snug">{{ localName(i, locale) }} <span v-if="i.spicy" class="text-xs" :title="$t('restaurant.menu.spicy')">{{ spicy(i.spicy) }}</span></h3>
            <p v-if="locale === 'lo' && i.name_lo && i.name_lo !== i.name" class="text-xs text-muted">{{ i.name }}</p>
            <p class="mt-0.5 line-clamp-2 text-xs text-muted">{{ i.description }}</p>
            <div class="mt-auto flex items-center justify-between gap-2 pt-2">
              <b>{{ money(i.price_cents, cur) }}</b>
              <span v-if="!i.available" class="chip bg-night/80 text-white">{{ $t('restaurant.menu.soldOut') }}</span>
              <div v-else-if="basket[i.id]?.qty" class="flex items-center rounded-xl bg-field">
                <button type="button" class="px-3 py-1.5" :aria-label="$t('restaurant.menu.less')" @click="add(i, -1)">−</button>
                <span class="w-6 text-center font-semibold">{{ basket[i.id]!.qty }}</span>
                <button type="button" class="px-3 py-1.5" :aria-label="$t('restaurant.menu.more')" @click="add(i)">+</button>
              </div>
              <UButton v-else size="sm" icon="i-lucide-plus" :disabled="!settings?.accepting_orders" @click="add(i)">{{ $t('restaurant.menu.add') }}</UButton>
            </div>
          </div>
        </article>
      </div>
    </section>

    <!-- basket bar -->
    <div v-if="count" class="fixed inset-x-3 bottom-24 z-30 mx-auto max-w-3xl md:bottom-6">
      <button type="button" class="flex w-full items-center gap-3 rounded-2xl bg-brand-500 px-5 py-4 text-left font-bold text-white shadow-[0_14px_30px_-12px_var(--brand-500)]" data-testid="basket" @click="checkout = true">
        <span class="grid size-7 place-items-center rounded-full bg-white/25 text-sm">{{ count }}</span>
        <span class="flex-1">{{ $t('restaurant.menu.viewBasket') }}</span>
        <span>{{ money(subtotal + service, cur) }}</span>
      </button>
    </div>

    <UModal v-model:open="checkout" :title="$t('restaurant.order.title')">
      <template #body>
        <form class="space-y-4" @submit.prevent="place">
          <div v-if="modes.length > 1" class="grid grid-cols-3 gap-2" role="radiogroup">
            <button v-for="m in modes" :key="m" type="button" role="radio" :aria-checked="mode === m" class="rounded-xl border-2 p-2 text-sm font-semibold" :class="mode === m ? 'border-brand-500 bg-brand-500/10 text-brand-600' : 'border-line'" @click="mode = m">
              {{ $t(`restaurant.mode.${m}`) }}
            </button>
          </div>
          <p v-else-if="modes.length" class="text-sm font-semibold">{{ $t(`restaurant.mode.${mode}`) }}<template v-if="mode === 'dine_in' && table"> · {{ $t('restaurant.menu.table', { label: table.label }) }}</template></p>
          <p v-if="!table && settings?.dine_in" class="text-xs text-muted">{{ $t('restaurant.order.scanHint') }}</p>

          <ul class="divide-y divide-line rounded-2xl bg-paper px-3">
            <li v-for="l in lines" :key="l.item.id" class="py-2.5 text-sm">
              <div class="flex items-center gap-2">
                <span class="font-semibold">{{ l.qty }} ×</span>
                <span class="flex-1">{{ localName(l.item, locale) }}</span>
                <span>{{ money(l.qty * l.item.price_cents, cur) }}</span>
              </div>
              <input v-model="basket[l.item.id]!.note" class="mt-1 w-full rounded-lg bg-transparent text-xs text-muted outline-none placeholder:text-muted/70" maxlength="120" :placeholder="$t('restaurant.order.itemNote')">
            </li>
          </ul>
          <div class="space-y-1 text-sm">
            <div class="flex justify-between"><span>{{ $t('common.subtotal') }}</span><span>{{ money(subtotal, cur) }}</span></div>
            <div v-if="service" class="flex justify-between"><span>{{ $t('restaurant.order.service', { pct: pct(settings?.service_charge_bps) }) }}</span><span>{{ money(service, cur) }}</span></div>
            <div class="flex justify-between text-base font-extrabold"><span>{{ $t('common.total') }}</span><span>{{ money(subtotal + service, cur) }}</span></div>
          </div>

          <div class="grid gap-3 sm:grid-cols-2">
            <div><label class="label" for="ro-name">{{ $t('restaurant.order.name') }}</label><UInput id="ro-name" v-model="form.customer_name" maxlength="80" /></div>
            <div><label class="label" for="ro-phone">{{ $t('restaurant.order.phone') }}<span v-if="mode !== 'dine_in'"> *</span></label><UInput id="ro-phone" v-model="form.customer_phone" type="tel" :required="mode !== 'dine_in'" placeholder="020 5555 1234" /></div>
          </div>
          <div v-if="mode === 'delivery'"><label class="label" for="ro-addr">{{ $t('restaurant.order.address') }} *</label><UTextarea id="ro-addr" v-model="form.address" :rows="2" required /></div>
          <div><label class="label" for="ro-note">{{ $t('restaurant.order.note') }}</label><UInput id="ro-note" v-model="form.note" maxlength="300" :placeholder="$t('restaurant.order.notePlaceholder')" /></div>
          <p v-if="error" class="text-sm text-brand-700" role="alert">{{ error }}</p>
          <UButton type="submit" block :loading="sending" :disabled="!modes.length || !settings?.accepting_orders">{{ $t('restaurant.order.place', { total: money(subtotal + service, cur) }) }}</UButton>
        </form>
      </template>
    </UModal>
  </div>
</template>
