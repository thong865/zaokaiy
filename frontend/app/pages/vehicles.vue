<script setup lang="ts">
useHead({ title: () => `${tr('vehicle.showroom.heroTitle')} · zaokaiy` })
</script>

<template>
  <div class="mx-auto max-w-7xl px-4 pt-10">
    <div class="card overflow-hidden bg-night p-8 text-white">
      <h1 class="text-3xl font-extrabold tracking-tight">{{ $t('vehicle.showroom.heroTitle') }}</h1>
      <p class="mt-2 max-w-xl text-white/75">{{ $t('vehicle.showroom.heroText') }}</p>
      <UButton color="neutral" variant="ghost" size="sm" to="/dashboard" class="mt-4 rounded-xl bg-white px-4 font-semibold text-night">{{ $t('vehicle.showroom.sellYours') }}</UButton>
    </div>
    <div class="mt-8"><VehicleShowroom /></div>
  </div>
</template>
