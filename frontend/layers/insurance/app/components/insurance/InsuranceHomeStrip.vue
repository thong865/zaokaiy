<script setup lang="ts">
/** Home screen section: insurance types → plans. */
import { KIND_ICON } from '#insurance/utils/insurance'

const route = useRoute()
const kinds = ['motor', 'travel', 'health', 'property']
</script>

<template>
  <section v-if="!route.query.q && !route.query.category" class="mx-auto max-w-7xl px-4 pt-10">
    <div class="card flex flex-wrap items-center gap-4 p-5">
      <div class="min-w-48 flex-1">
        <h2 class="text-xl font-extrabold tracking-tight">{{ $t('insurance.public.homeTitle') }}</h2>
        <p class="text-sm text-muted">{{ $t('insurance.public.homeText') }}</p>
      </div>
      <div class="flex flex-wrap gap-2">
        <UButton v-for="k in kinds" :key="k" color="neutral" variant="soft" :icon="`i-lucide-${KIND_ICON[k]}`" :to="{ path: '/insurance', query: { kind: k } }">{{ $t(`insurance.kind.${k}`) }}</UButton>
      </div>
    </div>
  </section>
</template>
