<script setup lang="ts">
/** Approve a POS terminal's pairing code (opened from the desktop app, or typed in). */
definePageMeta({ middleware: 'auth' })
const route = useRoute()
const api = useApi()
const { t } = useI18n()
const { shops, loadShops, shopId } = useShop()
useHead({ title: () => `${t('possync.pair.title')} · zaokaiy` })

interface PairInfo { code: string; name: string; platform: string; status: 'pending' | 'approved' | 'expired' | 'used'; shop: string | null }
const code = ref(String(route.query.code ?? ''))
const info = ref<PairInfo | null>(null)
const shop = ref('')
const error = ref('')
const busy = ref(false)
const done = ref('')

await loadShops().catch(() => {})
const posShops = computed(() => shops.value.filter((s) => allowed(s.access, 'pos')))
shop.value = posShops.value.find((s) => s.id === shopId.value)?.id ?? posShops.value[0]?.id ?? ''

async function lookup() {
  error.value = ''
  info.value = null
  if (!code.value.trim()) return
  busy.value = true
  try {
    info.value = await api<PairInfo>(`/pos/pair/${encodeURIComponent(code.value.trim())}`)
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
async function approve() {
  busy.value = true
  error.value = ''
  try {
    const r = await api<{ shop: { name: string } }>('/pos/pair/approve', { method: 'POST', body: { code: code.value.trim(), shop_id: shop.value } })
    done.value = r.shop.name
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
if (code.value) await lookup()
const platformIcon = (p: string) => (p === 'macos' ? 'i-lucide-laptop' : 'i-lucide-monitor')
</script>

<template>
  <div class="mx-auto grid max-w-md px-4 py-16">
    <div class="card space-y-5 p-6">
      <div class="flex items-center gap-3">
        <span class="icon-tile"><UIcon name="i-lucide-monitor-smartphone" class="size-5" /></span>
        <h1 class="text-xl font-extrabold">{{ $t('possync.pair.title') }}</h1>
      </div>

      <template v-if="done">
        <UAlert color="success" variant="soft" icon="i-lucide-circle-check" :description="$t('possync.pair.approved', { shop: done })" />
      </template>
      <template v-else>
        <p class="text-sm text-muted">{{ $t('possync.pair.lead') }}</p>
        <form class="flex gap-2" @submit.prevent="lookup">
          <UInput v-model="code" size="xl" class="flex-1" :placeholder="$t('possync.pair.code')" :ui="{ base: 'font-mono tracking-widest uppercase' }" autofocus />
          <UButton type="submit" size="xl" variant="soft" :loading="busy && !info">{{ $t('possync.pair.lookup') }}</UButton>
        </form>

        <template v-if="info">
          <UAlert v-if="info.status === 'expired'" color="warning" variant="soft" :description="$t('possync.pair.expired')" />
          <UAlert v-else-if="info.status !== 'pending'" color="warning" variant="soft" :description="$t('possync.pair.used')" />
          <template v-else>
            <div class="flex items-center gap-3 rounded-xl bg-[var(--field)] p-3">
              <UIcon :name="platformIcon(info.platform)" class="size-6 text-muted" />
              <div>
                <div class="text-xs text-muted">{{ $t('possync.pair.terminal') }}</div>
                <div class="font-semibold">{{ info.name }} <span class="text-xs font-normal text-muted">· {{ info.platform }} · {{ info.code }}</span></div>
              </div>
            </div>
            <p v-if="!posShops.length" class="text-sm text-error">{{ $t('possync.pair.noShops') }}</p>
            <template v-else>
              <UFormField :label="$t('possync.pair.shop')">
                <USelect v-model="shop" :items="posShops.map((s) => ({ label: s.name, value: s.id }))" class="w-full" size="lg" />
              </UFormField>
              <p class="text-xs text-muted">{{ $t('possync.pair.asYou') }}</p>
              <UButton size="xl" block icon="i-lucide-check" :loading="busy" :disabled="!shop" @click="approve">{{ $t('possync.pair.approve') }}</UButton>
            </template>
          </template>
        </template>
        <UAlert v-if="error" color="error" variant="soft" :description="error" />
      </template>
    </div>
  </div>
</template>
