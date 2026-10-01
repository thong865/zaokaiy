<script setup lang="ts">
definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const { shops, shop, createShop, can, access } = useShop()
const { t, locale } = useI18n()
// Each shop type's core supplies its onboarding choice and its overview (commerce: sales · restaurant: today's orders …).
const { overview, onboarding } = useCores()
const overviewFor = computed(() => overview(shop.value?.vertical))

const form = reactive({ name: '', slug: '', kind: 'seller', vertical: 'general', entity_type: 'individual', description: '', currency: locale.value === 'lo' ? 'LAK' : 'THB' })
// Onboarding choices come from the cores: seller · creator (commerce), vehicle dealer, restaurant, insurance agent …
const choice = ref('seller')
function pick(k: string) {
  const o = onboarding.value.find((x) => x.id === k)
  if (!o) return
  choice.value = k
  form.kind = o.kind ?? 'seller'
  form.vertical = o.vertical
  if (o.business) form.entity_type = 'business'
}
const kinds = computed(() => onboarding.value.map((o) => [o.id, t(o.label), t(o.hint)]))
const error = ref('')
watch(() => form.name, (n) => {
  form.slug = n.toLowerCase().normalize('NFKD').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 40)
})
async function create() {
  error.value = ''
  try {
    await createShop({ ...form } as never)
    if (form.entity_type === 'business') await navigateTo('/dashboard/verification')
  } catch (e) {
    error.value = apiError(e)
  }
}
</script>

<template>
  <div>
    <div v-if="!shops.length" class="mx-auto max-w-xl py-10">
      <h1 class="page-title text-3xl">{{ $t('dash.open.title') }}</h1>
      <p class="mt-1 text-muted">{{ $t('dash.open.subtitle') }}</p>
      <form class="card mt-8 space-y-4 p-6" @submit.prevent="create">
        <div class="grid gap-3 sm:grid-cols-3">
          <button v-for="k in kinds" :key="k[0]" type="button" class="rounded-2xl border-2 p-4 text-left" :class="choice === k[0] ? 'border-ink bg-paper' : 'border-line'" @click="pick(k[0]!)">
            <div class="font-bold">{{ k[1] }}</div><div class="text-xs text-muted">{{ k[2] }}</div>
          </button>
        </div>
        <label class="flex items-start gap-3 rounded-2xl border border-line p-4 text-sm">
          <input v-model="form.entity_type" type="checkbox" true-value="business" false-value="individual" class="mt-0.5 size-5 accent-brand-500">
          <span><b>{{ $t('kyb.entity.openLabel') }}</b><br><span class="text-muted">{{ $t('kyb.entity.openHint') }}</span></span>
        </label>
        <div><label class="label">{{ $t('dash.open.shopName') }}</label><UInput v-model="form.name" required /></div>
        <div>
          <label class="label">{{ $t('dash.open.shopUrl') }}</label>
          <div class="flex items-center gap-2 text-sm"><span class="text-muted">zaokaiy/s/</span><UInput v-model="form.slug" required pattern="[a-z0-9\-]{3,40}" /></div>
        </div>
        <div><label class="label">{{ $t('dash.open.about') }}</label><UTextarea v-model="form.description" :rows="3" /></div>
        <div>
          <label class="label">{{ $t('dash.settings.currency') }}</label>
          <select v-model="form.currency" class="input">
            <option v-for="c in ['THB', 'LAK', 'USD']" :key="c" :value="c">{{ $t(`dash.settings.currencies.${c}`) }}</option>
          </select>
        </div>
        <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
        <UButton type="submit" class="w-full">{{ $t('dash.open.create') }}</UButton>
      </form>
    </div>

    <!-- staff whose role has no page at all -->
    <div v-else-if="!can('stats')" class="mx-auto max-w-xl py-10">
      <EmptyState icon="i-lucide-user-round-check" :title="$t('staff.noAccessTitle', { shop: shop?.name ?? '' })" :text="$t('staff.noAccessText', { role: access.role })" />
    </div>
    <component :is="overviewFor" v-else-if="overviewFor" :key="shop?.id" />
    <div v-else-if="shop" class="card p-6">
      <h1 class="page-title">{{ $t('dash.overview.hello', { name: shop.name }) }}</h1>
      <p class="mt-1 text-sm text-muted">{{ $t('dash.overview.subtitle') }}</p>
    </div>
  </div>
</template>
