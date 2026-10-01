<script setup lang="ts">
import type { Payment } from '~/utils/types'

const props = defineProps<{ total: number; currency: string; busy?: boolean; error?: string }>()
const emit = defineEmits<{ complete: [payments: Payment[]] }>()
const open = defineModel<boolean>('open', { default: false })
const { t } = usePos()

const METHODS: Payment['method'][] = ['cash', 'card', 'transfer', 'qr', 'other']
const ICONS: Record<Payment['method'], string> = {
  cash: 'i-lucide-banknote',
  card: 'i-lucide-credit-card',
  transfer: 'i-lucide-landmark',
  qr: 'i-lucide-qr-code',
  other: 'i-lucide-wallet',
}
const method = ref<Payment['method']>('cash')
const amount = ref('')
const reference = ref('')
const payments = ref<Payment[]>([])
const input = ref<{ inputRef?: HTMLInputElement } | null>(null)

const paid = computed(() => payments.value.reduce((s, p) => s + p.amount_cents, 0))
const remaining = computed(() => Math.max(0, props.total - paid.value))
const typed = computed(() => toCents(amount.value))
const cashIn = computed(() => payments.value.filter((p) => p.method === 'cash').reduce((s, p) => s + p.amount_cents, 0))
const change = computed(() => Math.max(0, paid.value - props.total))
const quick = computed(() => quickCash(remaining.value, props.currency))

watch(open, (v) => {
  if (!v) return
  payments.value = []
  method.value = 'cash'
  amount.value = ''
  reference.value = ''
  nextTick(() => input.value?.inputRef?.focus())
})

// Keep the amount field focused so Enter always completes / adds the payment.
watch(method, () => nextTick(() => input.value?.inputRef?.focus()))

function add(cents: number) {
  if (cents <= 0) return false
  // Card / transfer / QR can't be more than what is still due (change only comes from cash).
  const value = method.value === 'cash' ? cents : Math.min(cents, remaining.value)
  if (value <= 0) return false
  payments.value.push({ method: method.value, amount_cents: value, reference: method.value === 'cash' ? '' : reference.value.trim() })
  amount.value = ''
  reference.value = ''
  return true
}

function submit() {
  if (typed.value > 0) add(typed.value)
  else if (remaining.value > 0 && method.value !== 'cash') add(remaining.value)
  else if (remaining.value > 0 && !payments.value.length) add(remaining.value) // Enter on an empty field = exact cash
  if (paid.value >= props.total && change.value <= cashIn.value) emit('complete', [...payments.value])
}

function quickPay(cents: number) {
  method.value = 'cash'
  add(cents)
  submit()
}
</script>

<template>
  <UModal v-model:open="open" :title="t('pay.title')" :ui="{ content: 'max-w-2xl' }">
    <template #body>
      <div class="grid grid-cols-[1fr_15rem] gap-6">
        <div class="space-y-4">
          <div class="grid grid-cols-5 gap-2">
            <button
              v-for="m in METHODS"
              :key="m"
              type="button"
              :class="['flex flex-col items-center gap-1 rounded-xl border-2 py-3 text-xs font-semibold transition', method === m ? 'border-brand-500 bg-brand-50 text-brand-600' : 'border-transparent bg-field hover:bg-[var(--field-hover)]']"
              @click="method = m"
            >
              <UIcon :name="ICONS[m]" class="size-5" />{{ t(`pay.method.${m}`) }}
            </button>
          </div>
          <form class="space-y-3" @submit.prevent="submit">
            <!-- default button: Enter submits even when the reference field is shown -->
            <button type="submit" class="sr-only" tabindex="-1" aria-hidden="true" />
            <UFormField :label="t('pay.received')">
              <UInput ref="input" v-model="amount" size="xl" inputmode="decimal" class="w-full" :placeholder="num(remaining)" :ui="{ base: 'text-2xl font-bold tabular-nums' }" />
            </UFormField>
            <UInput v-if="method !== 'cash'" v-model="reference" :placeholder="t('pay.reference')" class="w-full" />
            <div v-if="method === 'cash'" class="flex flex-wrap gap-2">
              <UButton v-for="q in quick" :key="q" color="neutral" variant="soft" size="lg" class="tabular-nums" @click="quickPay(q)">
                {{ q === remaining ? t('pay.exact') : money(q, currency) }}
              </UButton>
            </div>
            <UButton v-if="remaining > 0 && payments.length" type="button" variant="soft" icon="i-lucide-plus" @click="add(typed || remaining)">{{ t('pay.addPayment') }}</UButton>
          </form>
        </div>

        <div class="flex flex-col rounded-2xl bg-surface-2 p-4">
          <div class="eyebrow">{{ t('pay.due') }}</div>
          <div class="text-3xl font-extrabold tabular-nums">{{ money(total, currency) }}</div>
          <ul class="mt-4 flex-1 space-y-1.5 text-sm">
            <li v-for="(p, i) in payments" :key="i" class="flex items-center gap-2">
              <UIcon :name="ICONS[p.method]" class="size-4 text-muted" />
              <span class="flex-1">{{ t(`pay.method.${p.method}`) }}</span>
              <span class="tabular-nums">{{ money(p.amount_cents, currency) }}</span>
              <button type="button" class="text-muted hover:text-error" @click="payments.splice(i, 1)"><UIcon name="i-lucide-x" class="size-3.5" /></button>
            </li>
          </ul>
          <div class="mt-3 space-y-1 border-t border-line pt-3 text-sm">
            <div class="flex justify-between"><span class="text-muted">{{ t('pay.paid') }}</span><span class="tabular-nums">{{ money(paid, currency) }}</span></div>
            <div v-if="remaining > 0" class="flex justify-between font-semibold text-error"><span>{{ t('pay.remaining') }}</span><span class="tabular-nums">{{ money(remaining, currency) }}</span></div>
            <div v-else class="flex justify-between text-lg font-extrabold text-mint-500"><span>{{ t('pay.change') }}</span><span class="tabular-nums">{{ money(change, currency) }}</span></div>
          </div>
        </div>
      </div>
      <UAlert v-if="error" class="mt-4" color="error" variant="soft" :description="error" icon="i-lucide-circle-alert" />
    </template>
    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton color="neutral" variant="ghost" @click="open = false">{{ t('pay.cancel') }}</UButton>
        <UButton size="lg" icon="i-lucide-check" :loading="busy" @click="submit">
          {{ t('pay.complete') }}
        </UButton>
      </div>
    </template>
  </UModal>
</template>
