<script setup lang="ts">
/** The signed-in customer's insurance applications and policies. */
import type { Application } from '#insurance/utils/insurance'

definePageMeta({ middleware: 'auth' })
const api = useApi()
const { data: apps } = await useAsyncData('my-insurance', () => api<Application[]>('/me/insurance'), { default: () => [] })
</script>

<template>
  <div class="mx-auto max-w-3xl px-4 pt-10">
    <h1 class="page-title">{{ $t('insurance.my.title') }}</h1>
    <EmptyState v-if="!apps.length" class="mt-6" :title="$t('insurance.my.emptyTitle')" :text="$t('insurance.my.emptyText')"><UButton to="/insurance">{{ $t('insurance.public.browse') }}</UButton></EmptyState>
    <div class="mt-6 space-y-3">
      <article v-for="a in apps" :key="a.id" class="card p-5">
        <div class="flex flex-wrap items-start justify-between gap-2">
          <div>
            <div class="text-xs text-muted">{{ a.number }} · {{ a.insurer }}</div>
            <h2 class="font-bold">{{ a.plan_name }}</h2>
            <div class="text-sm text-muted">{{ a.shop_name }}</div>
          </div>
          <InsuranceStatusBadge :status="a.status" />
        </div>
        <div class="mt-3 grid grid-cols-2 gap-2 text-sm sm:grid-cols-4">
          <div><div class="text-xs text-muted">{{ $t('insurance.apply.quote') }}</div><b>{{ money(a.premium_cents, a.currency) }}</b></div>
          <div><div class="text-xs text-muted">{{ $t('insurance.my.period') }}</div><b>{{ fmtDate(a.start_date, 'short') }} – {{ fmtDate(a.end_date, 'short') }}</b></div>
          <div><div class="text-xs text-muted">{{ $t('insurance.my.payment') }}</div><b>{{ a.paid ? $t('insurance.my.paid') : $t('insurance.my.unpaid') }}</b></div>
          <div v-if="a.policy_no"><div class="text-xs text-muted">{{ $t('insurance.my.policyNo') }}</div><b>{{ a.policy_no }}</b></div>
        </div>
        <p v-if="a.decision_note" class="mt-3 rounded-xl bg-paper p-3 text-sm">{{ a.decision_note }}</p>
      </article>
    </div>
  </div>
</template>
