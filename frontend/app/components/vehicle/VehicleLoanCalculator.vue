<script setup lang="ts">
const props = defineProps<{ price: number; currency: string }>()
const down = ref(30)
const months = ref(48)
const rate = ref(7)
const terms = [12, 24, 36, 48, 60, 72, 84]
const r = computed(() => flatLoan(props.price, Number(down.value), Number(months.value), Number(rate.value) || 0))
</script>

<template>
  <section class="card p-5">
    <h2 class="font-bold">{{ $t('vehicle.loan.title') }}</h2>
    <div class="mt-4 space-y-4">
      <div>
        <div class="flex items-center justify-between text-sm"><label for="loan-down" class="font-semibold">{{ $t('vehicle.loan.down') }}</label><span>{{ down }}% · {{ money(r.down, currency) }}</span></div>
        <input id="loan-down" v-model.number="down" type="range" min="0" max="90" step="5" class="mt-1 w-full accent-brand-500">
      </div>
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label for="loan-term" class="label">{{ $t('vehicle.loan.term') }}</label>
          <select id="loan-term" v-model.number="months" class="input"><option v-for="m in terms" :key="m" :value="m">{{ $t('vehicle.unit.months', { n: m }) }}</option></select>
        </div>
        <div>
          <label for="loan-rate" class="label">{{ $t('vehicle.loan.rate') }}</label>
          <UInput id="loan-rate" v-model.number="rate" type="number" min="0" max="40" step="0.1" />
        </div>
      </div>
      <div class="rounded-2xl bg-ink p-4 text-paper">
        <div class="text-xs uppercase tracking-wider opacity-70">{{ $t('vehicle.loan.monthly') }}</div>
        <div class="text-2xl font-extrabold" data-testid="loan-monthly">{{ money(r.monthly, currency) }}</div>
        <div class="mt-2 grid grid-cols-2 gap-2 text-xs opacity-80">
          <div>{{ $t('vehicle.loan.financed') }}<br><b>{{ money(r.principal, currency) }}</b></div>
          <div>{{ $t('vehicle.loan.totalInterest') }}<br><b>{{ money(r.interest, currency) }}</b></div>
        </div>
      </div>
      <p class="text-xs text-muted">{{ $t('vehicle.loan.note') }}</p>
    </div>
  </section>
</template>
