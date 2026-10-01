<script setup lang="ts">
/** Staff invite link: see which shop and role it is for, then join (sign in or register first). */
const route = useRoute()
const api = useApi()
const { t } = useI18n()
const { loggedIn } = useAuth()
const { loadShops, select } = useShop()
const token = computed(() => String(route.params.token))
interface Info { shop: { id: string; name: string; slug: string; logo_url: string | null }; role: string; permissions: string[]; expires_at: string; state: 'open' | 'used' | 'expired' | 'revoked' }
const { data: info, error: loadError } = await useAsyncData(`join-${token.value}`, () => api<Info>(`/join/${token.value}`))
useHead({ title: () => `${t('staff.join.title')} · zaokaiy` })
const busy = ref(false)
const error = ref('')
async function join() {
  busy.value = true
  error.value = ''
  try {
    const r = await api<{ shop_id: string }>(`/join/${token.value}`, { method: 'POST' })
    await loadShops(true)
    select(r.shop_id)
    await navigateTo('/dashboard')
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
const next = computed(() => encodeURIComponent(route.fullPath))
</script>

<template>
  <div class="mx-auto grid max-w-md px-4 py-16">
    <div class="card space-y-5 p-6 text-center">
      <template v-if="info">
        <UAvatar :src="info.shop.logo_url ?? undefined" :text="info.shop.name.slice(0, 1).toUpperCase()" size="3xl" class="mx-auto bg-primary" :ui="{ fallback: 'text-white font-bold' }" />
        <div>
          <p class="text-sm text-muted">{{ $t('staff.join.invited') }}</p>
          <h1 class="page-title">{{ info.shop.name }}</h1>
          <p class="mt-1">{{ $t('staff.join.as', { role: info.role }) }}</p>
        </div>
        <div class="flex flex-wrap justify-center gap-1">
          <span v-for="p in info.permissions" :key="p" class="chip bg-[var(--field)]">{{ $t(`staff.perm.${p}`) }}</span>
        </div>
        <template v-if="info.state === 'open'">
          <template v-if="loggedIn">
            <UButton size="lg" block icon="i-lucide-user-plus" :loading="busy" @click="join">{{ $t('staff.join.accept') }}</UButton>
          </template>
          <template v-else>
            <p class="text-sm text-muted">{{ $t('staff.join.signInFirst') }}</p>
            <div class="grid gap-2">
              <UButton size="lg" block :to="`/login?next=${next}`">{{ $t('auth.login.submit') }}</UButton>
              <UButton size="lg" block color="neutral" variant="soft" :to="`/register?next=${next}`">{{ $t('auth.login.createAccount') }}</UButton>
            </div>
          </template>
          <p class="text-xs text-muted">{{ $t('staff.join.expires', { date: fmtDate(info.expires_at) }) }}</p>
        </template>
        <UAlert v-else color="warning" variant="soft" icon="i-lucide-link-2-off" :title="$t(`staff.join.state.${info.state}`)" />
        <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
      </template>
      <EmptyState v-else-if="loadError" icon="i-lucide-link-2-off" :title="$t('staff.join.notFound')" :text="$t('staff.join.notFoundText')" />
    </div>
  </div>
</template>
