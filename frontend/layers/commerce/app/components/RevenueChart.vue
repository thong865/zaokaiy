<script setup lang="ts">
const props = defineProps<{ series: { date: string; revenue_cents: number }[]; currency?: string }>()
const max = computed(() => Math.max(1, ...props.series.map((d) => d.revenue_cents)))
const hover = ref<number | null>(null)
const current = computed(() => (hover.value === null ? undefined : props.series[hover.value]))
const label = (d: string) => fmtDayMonth(d)
</script>

<template>
  <div>
    <div class="mb-2 h-5 text-sm">
      <template v-if="current">
        <span class="font-bold">{{ money(current.revenue_cents, currency) }}</span>
        <span class="text-muted"> · {{ label(current.date) }}</span>
      </template>
      <span v-else class="text-muted">{{ $t('dash.overview.last14') }}</span>
    </div>
    <div class="flex h-40 items-end gap-1.5" @mouseleave="hover = null">
      <div
        v-for="(d, i) in series"
        :key="d.date"
        class="flex h-full flex-1 cursor-default items-end"
        @mouseenter="hover = i"
      >
        <div
          class="w-full rounded-t-md transition-colors"
          :class="hover === i ? 'bg-brand-600' : d.revenue_cents ? 'bg-brand-500' : 'bg-line'"
          :style="{ height: `${Math.max(3, (d.revenue_cents / max) * 100)}%` }"
        />
      </div>
    </div>
    <div class="mt-2 flex justify-between text-[11px] text-muted">
      <span>{{ series[0] && label(series[0].date) }}</span>
      <span>{{ series.at(-1) && label(series.at(-1)!.date) }}</span>
    </div>
  </div>
</template>
