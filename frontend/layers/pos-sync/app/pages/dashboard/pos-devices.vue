<script setup lang="ts">
/** Owner: the shop's POS terminals (desktop app) — last contact, uploads, remove. */
definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { t } = useI18n()
const { shopId } = useShop()
const toast = useToast()
useHead({ title: () => `${t('possync.devices.title')} · zaokaiy` })

interface Device { id: string; code: string; name: string; platform: string; app_version: string; user: string | null; created_at: string; last_seen_at: string | null; last_push_at: string | null; revoked_at: string | null; sales: number; last_receipt: string | null }
const { data: devices, refresh } = await useAsyncData('pos-devices', () => api<Device[]>(`/shops/${shopId.value}/pos-devices`), { watch: [shopId], default: () => [] as Device[] })
const origin = useRequestURL().origin
const pairUrl = `${origin}/pos-pair`

const target = ref<Device | null>(null)
async function revoke() {
  if (!target.value) return
  try {
    await api(`/shops/${shopId.value}/pos-devices/${target.value.id}/revoke`, { method: 'POST' })
    toast.add({ title: t('possync.devices.revoked'), color: 'success' })
    target.value = null
    await refresh()
  } catch (e) {
    toast.add({ title: apiError(e), color: 'error' })
  }
}
const when = (iso: string | null) => (iso ? ago(iso) : t('possync.devices.never'))
const online = (d: Device) => !d.revoked_at && d.last_seen_at && Date.now() - new Date(d.last_seen_at).getTime() < 2 * 60_000
</script>

<template>
  <div class="space-y-5">
    <div>
      <h1 class="page-title">{{ $t('possync.devices.title') }}</h1>
      <p class="mt-1 max-w-3xl text-sm text-muted">{{ $t('possync.devices.lead') }}</p>
    </div>
    <UAlert color="info" variant="soft" icon="i-lucide-download" :description="$t('possync.devices.download', { url: pairUrl })" />
    <div class="card overflow-x-auto">
      <table class="table">
        <thead>
          <tr>
            <th>{{ $t('possync.devices.code') }}</th>
            <th>{{ $t('possync.devices.name') }}</th>
            <th>{{ $t('possync.devices.cashier') }}</th>
            <th>{{ $t('possync.devices.lastSeen') }}</th>
            <th>{{ $t('possync.devices.lastUpload') }}</th>
            <th class="text-right">{{ $t('possync.devices.sales') }}</th>
            <th>{{ $t('possync.devices.lastReceipt') }}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          <tr v-for="d in devices" :key="d.id" :class="d.revoked_at && 'opacity-50'">
            <td class="font-bold">
              <span :class="['mr-1.5 inline-block size-2 rounded-full', online(d) ? 'bg-mint-500' : 'bg-ink/20']" />{{ d.code }}
            </td>
            <td>{{ d.name }} <span class="text-xs text-muted">· {{ d.platform }} {{ d.app_version }}</span></td>
            <td>{{ d.user }}</td>
            <td class="text-muted">{{ when(d.last_seen_at) }}</td>
            <td class="text-muted">{{ when(d.last_push_at) }}</td>
            <td class="text-right tabular-nums">{{ d.sales }}</td>
            <td class="font-mono text-xs">{{ d.last_receipt }}</td>
            <td class="text-right">
              <UBadge v-if="d.revoked_at" color="neutral" variant="soft">{{ $t('possync.devices.revoked') }}</UBadge>
              <UButton v-else size="xs" color="error" variant="soft" icon="i-lucide-unplug" @click="target = d">{{ $t('possync.devices.revoke') }}</UButton>
            </td>
          </tr>
          <tr v-if="!devices.length">
            <td colspan="8" class="py-10 text-center text-muted">{{ $t('possync.devices.empty') }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <UModal :open="!!target" :title="$t('possync.devices.revoke')" @update:open="(v: boolean) => !v && (target = null)">
      <template #body>
        <p class="text-sm">{{ $t('possync.devices.revokeConfirm', { code: target?.code ?? '' }) }}</p>
      </template>
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="ghost" @click="target = null">{{ $t('common.cancel') }}</UButton>
          <UButton color="error" @click="revoke">{{ $t('possync.devices.revoke') }}</UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
