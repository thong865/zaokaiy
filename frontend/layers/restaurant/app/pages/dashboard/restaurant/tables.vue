<script setup lang="ts">
/** Tables and their QR codes: customers scan to open the menu with the table attached. */
import type { DiningTable } from '#restaurant/utils/restaurant'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const { data, refresh, api, shopId } = useRestaurant()
const origin = useRequestURL().origin
const link = (t: DiningTable) => `${origin}/r/t/${t.token}`
const form = reactive({ label: '', seats: 4 })
const error = ref('')
async function run(fn: () => Promise<unknown>) {
  error.value = ''
  try {
    await fn()
    await refresh()
    return true
  } catch (e) {
    error.value = apiError(e)
    return false
  }
}
const add = () => run(() => api(`/shops/${shopId.value}/restaurant/tables`, { method: 'POST', body: form })).then((ok) => ok && Object.assign(form, { label: '', seats: 4 }))
const patch = (t: DiningTable, body: Record<string, unknown>) => run(() => api(`/restaurant/tables/${t.id}`, { method: 'PATCH', body }))
const remove = (t: DiningTable) => run(() => api(`/restaurant/tables/${t.id}`, { method: 'DELETE' }))
const printAll = () => window.print()
</script>

<template>
  <div v-if="data">
    <div class="flex flex-wrap items-end justify-between gap-3 print:hidden">
      <div>
        <h1 class="page-title">{{ $t('restaurant.tables.title') }}</h1>
        <p class="text-sm text-muted">{{ $t('restaurant.tables.subtitle') }}</p>
      </div>
      <UButton v-if="data.tables.length" color="neutral" variant="soft" icon="i-lucide-printer" @click="printAll">{{ $t('restaurant.tables.print') }}</UButton>
    </div>
    <form class="card mt-6 flex flex-wrap items-end gap-3 p-4 print:hidden" @submit.prevent="add">
      <div class="min-w-32 flex-1"><label class="label" for="tb-label">{{ $t('restaurant.tables.label') }}</label><UInput id="tb-label" v-model="form.label" required maxlength="80" placeholder="T1" /></div>
      <div class="w-28"><label class="label" for="tb-seats">{{ $t('restaurant.tables.seats') }}</label><UInput id="tb-seats" v-model.number="form.seats" type="number" min="1" max="100" /></div>
      <UButton type="submit">{{ $t('restaurant.tables.add') }}</UButton>
    </form>
    <p v-if="error" class="mt-3 text-sm text-brand-700" role="alert">{{ error }}</p>
    <EmptyState v-if="!data.tables.length" class="mt-6" :title="$t('restaurant.tables.emptyTitle')" :text="$t('restaurant.tables.emptyText')" />
    <div class="mt-6 grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-4 print:grid-cols-3">
      <article v-for="t in data.tables" :key="t.id" class="card flex flex-col items-center p-4 text-center print:break-inside-avoid print:shadow-none" :class="!t.active && 'opacity-50'">
        <div class="text-lg font-extrabold">{{ t.label }}</div>
        <div class="text-xs text-muted">{{ $t('restaurant.tables.seatsN', { n: t.seats }) }} · {{ data.shop.name }}</div>
        <RestaurantQrCode :value="link(t)" :label="$t('restaurant.tables.qrFor', { label: t.label })" class="mt-3 size-36" />
        <div class="mt-2 text-[11px] text-muted print:block">{{ $t('restaurant.tables.scan') }}</div>
        <div class="mt-3 flex flex-wrap justify-center gap-1 print:hidden">
          <UButton size="xs" color="neutral" variant="soft" :to="link(t)" target="_blank">{{ $t('restaurant.tables.test') }}</UButton>
          <UButton size="xs" color="neutral" variant="soft" @click="patch(t, { active: !t.active })">{{ t.active ? $t('restaurant.tables.disable') : $t('restaurant.tables.enable') }}</UButton>
          <UButton size="xs" color="neutral" variant="soft" @click="patch(t, { new_code: true })">{{ $t('restaurant.tables.newCode') }}</UButton>
          <UButton size="xs" color="neutral" variant="ghost" icon="i-lucide-trash-2" :aria-label="$t('common.delete')" @click="remove(t)" />
        </div>
      </article>
    </div>
  </div>
</template>
