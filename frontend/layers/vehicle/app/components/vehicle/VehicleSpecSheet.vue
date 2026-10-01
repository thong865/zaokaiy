<script setup lang="ts">
import type { VehicleSpec } from '~/utils/types'

const props = defineProps<{ spec: VehicleSpec }>()
const { t } = useI18n()
const rows = computed(() => {
  const s = props.spec
  const r: [string, string][] = [
    [t('vehicle.spec.type'), optLabel('type', s.vehicle_type)],
    [t('vehicle.spec.condition'), optLabel('condition', s.condition)],
    [t('vehicle.spec.make'), s.make],
    [t('vehicle.spec.model'), s.model],
    [t('vehicle.spec.variant'), s.variant],
    [t('vehicle.spec.year'), String(s.year)],
    [t('vehicle.spec.mileage'), km(s.mileage_km)],
    [t('vehicle.spec.fuel'), optLabel('fuel', s.fuel)],
    [t('vehicle.spec.transmission'), optLabel('transmission', s.transmission)],
    [t('vehicle.spec.body'), optLabel('body', s.body_type)],
    [t('vehicle.spec.drive'), optLabel('drive', s.drive)],
    [t('vehicle.spec.engine'), s.engine_cc ? t('vehicle.unit.cc', { n: num(s.engine_cc) }) : ''],
    [t('vehicle.spec.power'), s.power_hp ? t('vehicle.unit.hp', { n: s.power_hp }) : ''],
    [t('vehicle.spec.seats'), s.seats ? String(s.seats) : ''],
    [t('vehicle.spec.doors'), s.doors ? String(s.doors) : ''],
    [t('vehicle.spec.color'), s.color],
    [t('vehicle.spec.owners'), s.owners != null && s.condition !== 'new' ? String(s.owners) : ''],
    [t('vehicle.spec.registered'), s.registered ? t('vehicle.spec.registeredYes') : t('vehicle.spec.registeredNo')],
    [t('vehicle.spec.plateProvince'), s.plate_province],
    [t('vehicle.spec.vin'), s.vin_last4 ? `•••• ${s.vin_last4}` : ''],
    [t('vehicle.spec.warranty'), s.warranty_months ? t('vehicle.unit.months', { n: s.warranty_months }) : ''],
    [t('vehicle.spec.location'), s.location],
  ]
  return r.filter(([, v]) => v)
})
</script>

<template>
  <section class="card p-5">
    <h2 class="font-bold">{{ $t('vehicle.spec.title') }}</h2>
    <dl class="mt-3 grid gap-x-6 text-sm sm:grid-cols-2">
      <div v-for="[k, v] in rows" :key="k" class="flex justify-between gap-4 border-b border-line/70 py-2">
        <dt class="text-muted">{{ k }}</dt><dd class="text-right font-semibold">{{ v }}</dd>
      </div>
    </dl>
    <template v-if="spec.features.length">
      <h3 class="mt-5 text-sm font-bold">{{ $t('vehicle.spec.features') }}</h3>
      <ul class="mt-2 flex flex-wrap gap-1.5">
        <li v-for="f in spec.features" :key="f" class="chip bg-paper text-ink/80">✓ {{ f }}</li>
      </ul>
    </template>
  </section>
</template>
