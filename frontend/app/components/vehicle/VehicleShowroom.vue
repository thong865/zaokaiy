<script setup lang="ts">
import type { VehicleSearch } from '~/utils/types'

/** Vehicle search with filters (make → model, year, price, mileage, fuel…), synced to the URL. */
const props = defineProps<{ shopSlug?: string }>()
const route = useRoute()
const router = useRouter()
const api = useApi()
const { t } = useI18n()

const KEYS = ['type', 'make', 'model', 'year_min', 'year_max', 'price_min', 'price_max', 'km_max', 'fuel', 'transmission', 'condition', 'body', 'q', 'sort'] as const
type Key = (typeof KEYS)[number]
const qv = (k: Key) => (typeof route.query[k] === 'string' ? (route.query[k] as string) : '')
const filters = computed(() => Object.fromEntries(KEYS.map((k) => [k, qv(k)])) as Record<Key, string>)
const PAGE = 24
const limit = ref(PAGE)
watch(filters, () => (limit.value = PAGE))

const apiQuery = computed(() => {
  const f = filters.value
  const out: Record<string, string | number> = { limit: limit.value }
  if (props.shopSlug) out.shop = props.shopSlug
  for (const k of KEYS) if (f[k]) out[k] = f[k]
  // Prices are typed in major units.
  for (const k of ['price_min', 'price_max'] as const) if (f[k]) out[k] = Math.round(Number(f[k]) * 100)
  return out
})
const { data, pending, error } = await useAsyncData(
  () => `vehicles:${props.shopSlug ?? '*'}:${JSON.stringify(apiQuery.value)}`,
  () => api<VehicleSearch>('/vehicles', { query: apiQuery.value }),
  { watch: [apiQuery] },
)
if (error.value && props.shopSlug) throw createError({ statusCode: 404, statusMessage: tr('store.shop.notFound'), fatal: true })
// Keep the last facets while a new page loads (no flicker).
const facets = computed(() => data.value?.facets)
const currency = computed(() => data.value?.shop?.currency ?? data.value?.items[0]?.currency ?? 'LAK')

function set(patch: Partial<Record<Key, string | number | null>>) {
  const q: Record<string, string> = {}
  for (const [k, v] of Object.entries({ ...route.query })) if (typeof v === 'string') q[k] = v
  for (const [k, v] of Object.entries(patch)) {
    if (v === null || v === undefined || v === '') delete q[k]
    else q[k] = String(v)
  }
  if ('make' in patch) delete q.model
  if ('type' in patch) {
    delete q.make
    delete q.model
    delete q.body
  }
  router.replace({ query: q })
}
const activeCount = computed(() => KEYS.filter((k) => k !== 'sort' && k !== 'type' && filters.value[k]).length)
function reset() {
  set(Object.fromEntries(KEYS.filter((k) => k !== 'sort' && k !== 'type').map((k) => [k, null])))
}
const models = computed(() => facets.value?.makes.find((m) => m.make.toLowerCase() === filters.value.make.toLowerCase())?.models ?? [])
const years = computed(() => {
  const now = new Date().getFullYear() + 1
  const lo = facets.value?.year.min ?? now - 20
  return Array.from({ length: Math.max(1, now - lo + 1) }, (_, i) => now - i)
})
const kmSteps = [10_000, 30_000, 50_000, 80_000, 100_000, 150_000, 200_000]
const typeTabs = computed(() => {
  const counts = facets.value?.types ?? []
  const total = counts.reduce((a, [, n]) => a + n, 0)
  return [{ v: '', label: t('vehicle.showroom.allTypes'), n: total }, ...counts.map(([v, n]) => ({ v, label: optLabel('types', v), n }))]
})
const sorts = ['newest', 'price_asc', 'price_desc', 'year_desc', 'km_asc']
const q = ref(filters.value.q)
watch(() => filters.value.q, (v) => (q.value = v))
const priceMin = ref(filters.value.price_min)
const priceMax = ref(filters.value.price_max)
watch(() => [filters.value.price_min, filters.value.price_max], ([a, b]) => {
  priceMin.value = a ?? ''
  priceMax.value = b ?? ''
})
const panel = ref(false)
watch(filters, () => (panel.value = false))
</script>

<template>
  <div>
    <div class="flex flex-wrap items-center gap-2">
      <button
        v-for="tb in typeTabs"
        :key="tb.v"
        class="chip border px-3 py-1.5"
        :class="filters.type === tb.v ? 'border-brand-500 bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]' : 'border-transparent bg-surface shadow-card hover:text-brand-500'"
        @click="set({ type: tb.v || null })"
      >
        {{ tb.label }} <span class="opacity-60">{{ tb.n }}</span>
      </button>
      <form class="ml-auto flex w-full gap-2 sm:w-auto" @submit.prevent="set({ q: q.trim() || null })">
        <UInput v-model="q" class="sm:w-64" :placeholder="$t('vehicle.showroom.search')" :aria-label="$t('vehicle.showroom.search')" />
        <UButton color="neutral" size="sm" type="submit">{{ $t('common.search') }}</UButton>
      </form>
    </div>

    <div class="mt-6 grid gap-8 lg:grid-cols-[250px_1fr]">
      <UButton color="neutral" variant="soft" type="submit" class="lg:hidden" @click="panel = !panel">
        {{ $t('vehicle.showroom.filters') }}<span v-if="activeCount"> ({{ activeCount }})</span>
      </UButton>
      <aside class="space-y-4 lg:sticky lg:top-24 lg:block lg:self-start" :class="panel ? 'block' : 'hidden'">
        <div>
          <label class="label">{{ $t('vehicle.showroom.make') }}</label>
          <select class="input" :value="filters.make" @change="set({ make: ($event.target as HTMLSelectElement).value || null })">
            <option value="">{{ $t('vehicle.showroom.anyMake') }}</option>
            <option v-for="m in facets?.makes ?? []" :key="m.make" :value="m.make">{{ m.make }} ({{ m.count }})</option>
          </select>
        </div>
        <div>
          <label class="label">{{ $t('vehicle.showroom.model') }}</label>
          <select class="input disabled:bg-paper" :disabled="!filters.make" :value="filters.model" @change="set({ model: ($event.target as HTMLSelectElement).value || null })">
            <option value="">{{ $t('vehicle.showroom.anyModel') }}</option>
            <option v-for="m in models" :key="m.model" :value="m.model">{{ m.model }} ({{ m.count }})</option>
          </select>
        </div>
        <div>
          <label class="label">{{ $t('vehicle.showroom.year') }}</label>
          <div class="grid grid-cols-2 gap-2">
            <select class="input" :value="filters.year_min" :aria-label="$t('vehicle.showroom.yearFrom')" @change="set({ year_min: ($event.target as HTMLSelectElement).value || null })">
              <option value="">{{ $t('vehicle.showroom.from') }}</option>
              <option v-for="y in years" :key="y" :value="y">{{ y }}</option>
            </select>
            <select class="input" :value="filters.year_max" :aria-label="$t('vehicle.showroom.yearTo')" @change="set({ year_max: ($event.target as HTMLSelectElement).value || null })">
              <option value="">{{ $t('vehicle.showroom.to') }}</option>
              <option v-for="y in years" :key="y" :value="y">{{ y }}</option>
            </select>
          </div>
        </div>
        <form @submit.prevent="set({ price_min: priceMin || null, price_max: priceMax || null })">
          <label class="label">{{ $t('vehicle.showroom.price', { currency }) }}</label>
          <div class="grid grid-cols-2 gap-2">
            <UInput v-model.number="priceMin" type="number" min="0" :placeholder="$t('vehicle.showroom.min')" :aria-label="$t('vehicle.showroom.priceMin')" @change="set({ price_min: priceMin || null })" />
            <UInput v-model.number="priceMax" type="number" min="0" :placeholder="$t('vehicle.showroom.max')" :aria-label="$t('vehicle.showroom.priceMax')" @change="set({ price_max: priceMax || null })" />
          </div>
        </form>
        <div>
          <label class="label">{{ $t('vehicle.showroom.mileage') }}</label>
          <select class="input" :value="filters.km_max" @change="set({ km_max: ($event.target as HTMLSelectElement).value || null })">
            <option value="">{{ $t('vehicle.showroom.anyMileage') }}</option>
            <option v-for="k in kmSteps" :key="k" :value="k">≤ {{ km(k) }}</option>
          </select>
        </div>
        <div v-for="g in (['condition', 'fuel', 'transmission', 'body_type'] as const)" v-show="(facets?.[g]?.length ?? 0) > 1 || filters[g === 'body_type' ? 'body' : g]" :key="g">
          <div class="label">{{ $t(`vehicle.showroom.${g}`) }}</div>
          <div class="flex flex-wrap gap-1.5">
            <button
              v-for="[val, n] in facets?.[g] ?? []"
              :key="val"
              class="chip border"
              :class="filters[g === 'body_type' ? 'body' : g] === val ? 'border-brand-500 bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]' : 'border-transparent bg-surface shadow-card hover:text-brand-500'"
              @click="set({ [g === 'body_type' ? 'body' : g]: filters[g === 'body_type' ? 'body' : g] === val ? null : val })"
            >
              {{ optLabel(g === 'body_type' ? 'body' : g, val) }} <span class="opacity-60">{{ n }}</span>
            </button>
          </div>
        </div>
        <button v-if="activeCount" class="text-sm font-semibold text-brand-600 hover:underline" @click="reset">{{ $t('vehicle.showroom.reset') }}</button>
      </aside>

      <div>
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div class="text-sm font-semibold" aria-live="polite">{{ $t('vehicle.showroom.results', { n: num(data?.total ?? 0) }, data?.total ?? 0) }}</div>
          <select class="input w-auto py-1.5 text-sm" :value="filters.sort || 'newest'" :aria-label="$t('vehicle.showroom.sort')" @change="set({ sort: ($event.target as HTMLSelectElement).value === 'newest' ? null : ($event.target as HTMLSelectElement).value })">
            <option v-for="s in sorts" :key="s" :value="s">{{ $t(`vehicle.showroom.sorts.${s}`) }}</option>
          </select>
        </div>
        <div v-if="data?.items.length" class="mt-4 grid gap-5 sm:grid-cols-2 xl:grid-cols-3" :class="pending && 'opacity-60 transition'">
          <VehicleCard v-for="v in data.items" :key="`${v.id}-${v.via_shop_id}`" :v="v" :show-shop="!shopSlug" />
        </div>
        <EmptyState v-else-if="!pending" class="mt-6" :title="$t('vehicle.showroom.empty')" :text="$t('vehicle.showroom.emptyHint')" />
        <div v-if="data && data.items.length < data.total" class="mt-8 text-center">
          <UButton color="neutral" variant="soft" type="submit" :disabled="pending" @click="limit += PAGE">{{ $t('vehicle.showroom.loadMore') }}</UButton>
        </div>
      </div>
    </div>
  </div>
</template>
