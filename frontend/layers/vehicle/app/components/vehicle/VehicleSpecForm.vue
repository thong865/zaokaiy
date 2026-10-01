<script setup lang="ts">
import type { VehicleDraft } from '~/utils/types'
/** Vehicle spec sheet editor used inside <ProductForm>. Numbers are kept as strings while typing. */

const v = defineModel<VehicleDraft>({ required: true })
defineProps<{ currency?: string; existing?: boolean }>()
const years = computed(() => {
  const now = new Date().getFullYear() + 1
  return Array.from({ length: 60 }, (_, i) => now - i)
})
const makes = computed(() => makesFor(v.value.vehicle_type))
const bodies = computed(() => bodiesFor(v.value.vehicle_type))
watch(() => v.value.vehicle_type, (t) => {
  if (v.value.body_type && !bodiesFor(t).includes(v.value.body_type)) v.value.body_type = ''
  if (t === 'motorbike') {
    v.value.drive = ''
    v.value.doors = ''
  }
})
const feature = ref('')
function addFeature() {
  const f = feature.value.trim().replace(/,$/, '')
  if (f && !v.value.features.some((x) => x.toLowerCase() === f.toLowerCase())) v.value.features.push(f)
  feature.value = ''
}
</script>

<template>
  <div class="space-y-4">
    <div class="grid gap-3 sm:grid-cols-3">
      <div>
        <label for="vh-type" class="label">{{ $t('vehicle.spec.type') }}</label>
        <select id="vh-type" v-model="v.vehicle_type" class="input"><option v-for="t in VEHICLE_TYPES" :key="t" :value="t">{{ optLabel('type', t) }}</option></select>
      </div>
      <div>
        <label for="vh-cond" class="label">{{ $t('vehicle.spec.condition') }}</label>
        <select id="vh-cond" v-model="v.condition" class="input"><option v-for="c in CONDITIONS" :key="c" :value="c">{{ optLabel('condition', c) }}</option></select>
      </div>
      <div>
        <label for="vh-year" class="label">{{ $t('vehicle.spec.year') }}</label>
        <select id="vh-year" v-model="v.year" required class="input"><option value="" disabled>{{ $t('vehicle.form.select') }}</option><option v-for="y in years" :key="y" :value="y">{{ y }}</option></select>
      </div>
      <div>
        <label for="vh-make" class="label">{{ $t('vehicle.spec.make') }}</label>
        <UInput id="vh-make" v-model="v.make" required list="vh-makes" :placeholder="$t('vehicle.form.makePh')" autocomplete="off" />
        <datalist id="vh-makes"><option v-for="m in makes" :key="m" :value="m" /></datalist>
      </div>
      <div>
        <label for="vh-model" class="label">{{ $t('vehicle.spec.model') }}</label>
        <UInput id="vh-model" v-model="v.model" required maxlength="60" :placeholder="$t('vehicle.form.modelPh')" />
      </div>
      <div>
        <label for="vh-variant" class="label">{{ $t('vehicle.spec.variant') }}</label>
        <UInput id="vh-variant" v-model="v.variant" maxlength="80" :placeholder="$t('vehicle.form.variantPh')" />
      </div>
      <div>
        <label for="vh-km" class="label">{{ $t('vehicle.form.mileage') }}</label>
        <UInput id="vh-km" v-model.number="v.mileage_km" type="number" min="0" max="5000000" />
      </div>
      <div>
        <label for="vh-fuel" class="label">{{ $t('vehicle.spec.fuel') }}</label>
        <select id="vh-fuel" v-model="v.fuel" class="input"><option v-for="f in FUELS" :key="f" :value="f">{{ optLabel('fuel', f) }}</option></select>
      </div>
      <div>
        <label for="vh-trans" class="label">{{ $t('vehicle.spec.transmission') }}</label>
        <select id="vh-trans" v-model="v.transmission" class="input"><option v-for="t in TRANSMISSIONS" :key="t" :value="t">{{ optLabel('transmission', t) }}</option></select>
      </div>
      <div>
        <label for="vh-body" class="label">{{ $t('vehicle.spec.body') }}</label>
        <select id="vh-body" v-model="v.body_type" class="input"><option value="">—</option><option v-for="b in bodies" :key="b" :value="b">{{ optLabel('body', b) }}</option></select>
      </div>
      <div v-if="v.vehicle_type !== 'motorbike'">
        <label for="vh-drive" class="label">{{ $t('vehicle.spec.drive') }}</label>
        <select id="vh-drive" v-model="v.drive" class="input"><option value="">—</option><option v-for="d in DRIVES" :key="d" :value="d">{{ optLabel('drive', d) }}</option></select>
      </div>
      <div>
        <label for="vh-cc" class="label">{{ $t('vehicle.form.engineCc') }}</label>
        <UInput id="vh-cc" v-model.number="v.engine_cc" type="number" min="0" max="20000" />
      </div>
      <div>
        <label for="vh-hp" class="label">{{ $t('vehicle.form.powerHp') }}</label>
        <UInput id="vh-hp" v-model.number="v.power_hp" type="number" min="0" max="3000" />
      </div>
      <div>
        <label for="vh-seats" class="label">{{ $t('vehicle.spec.seats') }}</label>
        <UInput id="vh-seats" v-model.number="v.seats" type="number" min="1" max="60" />
      </div>
      <div v-if="v.vehicle_type !== 'motorbike'">
        <label for="vh-doors" class="label">{{ $t('vehicle.spec.doors') }}</label>
        <UInput id="vh-doors" v-model.number="v.doors" type="number" min="0" max="10" />
      </div>
      <div>
        <label for="vh-color" class="label">{{ $t('vehicle.spec.color') }}</label>
        <UInput id="vh-color" v-model="v.color" maxlength="80" />
      </div>
      <div v-if="v.condition !== 'new'">
        <label for="vh-owners" class="label">{{ $t('vehicle.spec.owners') }}</label>
        <UInput id="vh-owners" v-model.number="v.owners" type="number" min="0" max="50" />
      </div>
      <div>
        <label for="vh-warranty" class="label">{{ $t('vehicle.form.warranty') }}</label>
        <UInput id="vh-warranty" v-model.number="v.warranty_months" type="number" min="0" max="240" />
      </div>
      <div>
        <label for="vh-loc" class="label">{{ $t('vehicle.spec.location') }}</label>
        <UInput id="vh-loc" v-model="v.location" list="vh-provinces" maxlength="80" />
        <datalist id="vh-provinces"><option v-for="p in LAO_PROVINCES" :key="p" :value="p" /></datalist>
      </div>
    </div>

    <div>
      <label for="vh-feature" class="label">{{ $t('vehicle.spec.features') }}</label>
      <div class="flex flex-wrap gap-1.5">
        <button v-for="(f, i) in v.features" :key="f" type="button" class="chip bg-paper" :aria-label="$t('common.remove')" @click="v.features.splice(i, 1)">{{ f }} ✕</button>
      </div>
      <UInput id="vh-feature" v-model="feature" class="mt-2" :placeholder="$t('vehicle.form.featuresPh')" @keydown.enter.prevent="addFeature" @blur="addFeature" />
    </div>

    <div class="rounded-2xl border border-line p-4">
      <div class="text-sm font-bold">{{ $t('vehicle.form.papers') }}</div>
      <label class="mt-2 flex items-center gap-2 text-sm"><input v-model="v.registered" type="checkbox" class="size-4 accent-brand-500"> {{ $t('vehicle.form.registered') }}</label>
      <div class="mt-3 grid gap-3 sm:grid-cols-3">
        <div>
          <label for="vh-prov" class="label">{{ $t('vehicle.spec.plateProvince') }}</label>
          <UInput id="vh-prov" v-model="v.plate_province" list="vh-provinces" maxlength="80" />
        </div>
        <div>
          <label for="vh-plate" class="label">{{ $t('vehicle.form.plateNo') }}</label>
          <UInput id="vh-plate" v-model="v.plate_no" maxlength="80" class="font-mono" />
        </div>
        <div>
          <label for="vh-vin" class="label">{{ $t('vehicle.form.vin') }}</label>
          <UInput id="vh-vin" v-model="v.vin" maxlength="24" class="font-mono uppercase" />
        </div>
      </div>
      <p class="mt-2 text-xs text-muted">🔒 {{ $t('vehicle.form.privateHint') }}</p>
    </div>

    <div class="rounded-2xl border border-line p-4">
      <div class="text-sm font-bold">{{ $t('vehicle.form.selling') }}</div>
      <div class="mt-3 grid gap-3 sm:grid-cols-2">
        <div>
          <label for="vh-deposit" class="label">{{ $t('vehicle.form.deposit', { currency: currency ?? 'LAK' }) }}</label>
          <UInput id="vh-deposit" v-model.number="v.deposit" type="number" min="0" />
          <p class="mt-1 text-xs text-muted">{{ $t('vehicle.form.depositHint') }}</p>
        </div>
        <div v-if="existing">
          <label for="vh-sale" class="label">{{ $t('vehicle.form.saleStatus') }}</label>
          <select id="vh-sale" v-model="v.sale_status" class="input"><option v-for="s in SALE_STATUSES" :key="s" :value="s">{{ optLabel('sale', s) }}</option></select>
        </div>
      </div>
      <div class="mt-3 space-y-2 text-sm">
        <label class="flex items-center gap-2"><input v-model="v.negotiable" type="checkbox" class="size-4 accent-brand-500"> {{ $t('vehicle.form.negotiable') }}</label>
        <label class="flex items-center gap-2"><input v-model="v.finance_available" type="checkbox" class="size-4 accent-brand-500"> {{ $t('vehicle.form.finance') }}</label>
        <label class="flex items-start gap-2"><input v-model="v.buy_online" type="checkbox" class="mt-0.5 size-4 accent-brand-500"> <span>{{ $t('vehicle.form.buyOnline') }}<br><span class="text-xs text-muted">{{ $t('vehicle.form.buyOnlineHint') }}</span></span></label>
      </div>
    </div>
  </div>
</template>
