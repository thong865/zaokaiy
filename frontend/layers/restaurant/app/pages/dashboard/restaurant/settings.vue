<script setup lang="ts">
/** Ordering settings: open / closed, dine-in · takeaway · delivery, service charge, preparation time. */
definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const { data, refresh, api, shopId } = useRestaurant()
const f = reactive({ accepting_orders: true, dine_in: true, takeaway: true, delivery: false, service_charge: 0 as number | string, prep_minutes: 15 as number | string })
watch(data, (d) => {
  if (d) Object.assign(f, { ...d.settings, service_charge: d.settings.service_charge_bps / 100 })
}, { immediate: true })
const msg = ref('')
const error = ref('')
async function save() {
  msg.value = error.value = ''
  try {
    await api(`/shops/${shopId.value}/restaurant`, {
      method: 'PUT',
      body: { accepting_orders: f.accepting_orders, dine_in: f.dine_in, takeaway: f.takeaway, delivery: f.delivery,
              service_charge_bps: Math.round(Number(f.service_charge || 0) * 100), prep_minutes: Number(f.prep_minutes || 0) },
    })
    await refresh()
    msg.value = tr('common.saved')
  } catch (e) {
    error.value = apiError(e)
  }
}
</script>

<template>
  <form class="max-w-2xl space-y-4" @submit.prevent="save">
    <h1 class="page-title">{{ $t('restaurant.settings.title') }}</h1>
    <div class="card space-y-4 p-6">
      <USwitch v-model="f.accepting_orders" :label="$t('restaurant.settings.open')" :description="$t('restaurant.settings.openHint')" />
      <div class="grid gap-3 sm:grid-cols-3">
        <USwitch v-model="f.dine_in" :label="$t('restaurant.mode.dine_in')" />
        <USwitch v-model="f.takeaway" :label="$t('restaurant.mode.takeaway')" />
        <USwitch v-model="f.delivery" :label="$t('restaurant.mode.delivery')" />
      </div>
      <div class="grid gap-3 sm:grid-cols-2">
        <div><label class="label" for="rs-svc">{{ $t('restaurant.settings.service') }}</label><UInput id="rs-svc" v-model.number="f.service_charge" type="number" min="0" max="30" step="0.5" /></div>
        <div><label class="label" for="rs-prep">{{ $t('restaurant.settings.prep') }}</label><UInput id="rs-prep" v-model.number="f.prep_minutes" type="number" min="0" max="240" /></div>
      </div>
    </div>
    <div class="flex items-center gap-3">
      <UButton type="submit">{{ $t('common.save') }}</UButton>
      <span v-if="msg" class="text-sm text-mint-500">{{ msg }}</span>
      <span v-if="error" class="text-sm text-brand-700" role="alert">{{ error }}</span>
    </div>
  </form>
</template>
