<script setup lang="ts">
import { KIND_ICON, planName, type Plan } from '#insurance/utils/insurance'

const props = defineProps<{ plan: Plan }>()
const { locale } = useI18n()
const cur = computed(() => props.plan.currency ?? 'LAK')
</script>

<template>
  <NuxtLink :to="`/insurance/${plan.id}`" class="group card card-hover flex flex-col p-5" :data-testid="`plan-${plan.name}`">
    <div class="flex items-start gap-3">
      <span class="icon-tile bg-violet-500/12 text-violet-600"><UIcon :name="`i-lucide-${KIND_ICON[plan.kind] ?? 'shield'}`" class="size-5" /></span>
      <div class="min-w-0">
        <div class="text-[11px] font-bold uppercase tracking-wider text-muted">{{ $t(`insurance.kind.${plan.kind}`) }} · {{ plan.insurer }}</div>
        <h3 class="font-bold leading-snug group-hover:text-brand-500">{{ planName(plan, locale) }}</h3>
      </div>
    </div>
    <ul v-if="plan.coverage.length" class="mt-3 space-y-1 text-sm text-ink/80">
      <li v-for="c in plan.coverage.slice(0, 3)" :key="c" class="flex gap-2"><UIcon name="i-lucide-check" class="mt-0.5 size-4 shrink-0 text-mint-500" />{{ c }}</li>
    </ul>
    <div class="mt-auto flex items-end justify-between gap-2 pt-4">
      <div>
        <div class="text-xs text-muted">{{ plan.premium_mode === 'rate' ? $t('insurance.plan.rateOf', { pct: pct(plan.rate_bps) }) : $t('insurance.plan.premium') }}</div>
        <div class="text-lg font-extrabold">
          <template v-if="plan.premium_mode === 'rate'">{{ $t('insurance.plan.fromPrice', { price: money(plan.min_premium_cents, cur) }) }}</template>
          <template v-else>{{ money(plan.premium_cents, cur) }}</template>
          <span class="text-xs font-semibold text-muted"> / {{ $t('insurance.plan.months', { n: plan.term_months }) }}</span>
        </div>
      </div>
      <div v-if="plan.shop_name" class="text-right text-xs text-muted">{{ plan.shop_name }}</div>
    </div>
  </NuxtLink>
</template>
