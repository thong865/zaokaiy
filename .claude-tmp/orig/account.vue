<script setup lang="ts">
import type { LoginIdentities, LoginProvider, SignInResponse } from '~/utils/types'

/** Account: profile summary, connected login methods (Google / Facebook / WhatsApp), password. */
definePageMeta({ middleware: 'auth' })
const { t } = useI18n()
const route = useRoute()
const api = useApi()
const { user, fetchMe, startOAuth } = useAuth()
useHead({ title: () => t('auth.account.title') + ' · zaokaiy' })

const { data, refresh } = await useAsyncData('auth-identities', () => api<LoginIdentities>('/auth/identities'))
const flash = ref(route.query.linked ? t('auth.account.connected', { provider: label(route.query.linked as string) }) : '')
const error = ref('')
const busy = ref<string | null>(null)
const waOpen = ref(false)
const avatarFailed = ref(false)

function label(p: string) {
  return p === 'google' ? 'Google' : p === 'facebook' ? 'Facebook' : p === 'whatsapp' ? 'WhatsApp' : p
}
const methods = computed(() =>
  (['google', 'facebook', 'whatsapp'] as LoginProvider[])
    .filter((p) => data.value?.providers[p] || data.value?.identities.some((i) => i.provider === p))
    .map((p) => ({ provider: p, identity: data.value?.identities.find((i) => i.provider === p) })),
)

async function connect(p: LoginProvider) {
  error.value = flash.value = ''
  if (p === 'whatsapp') {
    waOpen.value = true
    return
  }
  busy.value = p
  try {
    await startOAuth(p, { redirect: '/account', link: true })
  } catch (e) {
    error.value = apiError(e)
    busy.value = null
  }
}
async function disconnect(p: LoginProvider) {
  if (!confirm(t('auth.account.confirmDisconnect', { provider: label(p) }))) return
  error.value = flash.value = ''
  busy.value = p
  try {
    await api(`/auth/identities/${p}`, { method: 'DELETE' })
    await Promise.all([refresh(), fetchMe()])
    flash.value = t('auth.account.disconnected', { provider: label(p) })
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = null
  }
}
async function waDone(_r: SignInResponse) {
  waOpen.value = false
  await refresh()
  flash.value = t('auth.account.connected', { provider: 'WhatsApp' })
}

const pw = reactive({ current: '', password: '' })
const pwMsg = ref('')
const pwErr = ref('')
async function savePassword() {
  pwMsg.value = pwErr.value = ''
  try {
    await api('/auth/password', { method: 'POST', body: { current: pw.current || undefined, password: pw.password } })
    pw.current = pw.password = ''
    pwMsg.value = t('auth.account.passwordSaved')
    await refresh()
  } catch (e) {
    pwErr.value = apiError(e)
  }
}
</script>

<template>
  <div class="mx-auto max-w-2xl px-4 py-10">
    <h1 class="page-title">{{ $t('auth.account.title') }}</h1>

    <div class="card mt-6 flex items-center gap-4 p-5">
      <img v-if="user?.avatar_url && !avatarFailed" :src="user.avatar_url" alt="" class="size-14 rounded-full object-cover" referrerpolicy="no-referrer" @error="avatarFailed = true">
      <span v-else class="grid size-14 place-items-center rounded-full bg-ink text-xl font-bold text-paper">{{ user?.display_name?.slice(0, 1).toUpperCase() }}</span>
      <div class="min-w-0">
        <div class="text-lg font-extrabold">{{ user?.display_name }}</div>
        <div class="truncate text-sm text-muted">{{ [data?.email, data?.phone].filter(Boolean).join(' · ') || $t('auth.account.noContact') }}</div>
      </div>
    </div>

    <p v-if="flash" class="mt-4 rounded-xl bg-mint-500/10 px-4 py-2 text-sm font-semibold text-mint-500">{{ flash }}</p>
    <p v-if="error" class="mt-4 rounded-xl bg-brand-50 px-4 py-2 text-sm text-brand-700">{{ error }}</p>

    <section class="card mt-6 p-5">
      <h2 class="font-bold">{{ $t('auth.account.methods') }}</h2>
      <p class="text-sm text-muted">{{ $t('auth.account.methodsHint') }}</p>
      <ul class="mt-4 divide-y divide-line/70">
        <li class="flex items-center gap-3 py-3">
          <span class="grid size-8 place-items-center rounded-full bg-ink text-xs font-bold text-paper" aria-hidden="true">•••</span>
          <div class="min-w-0 flex-1">
            <div class="font-semibold">{{ $t('common.password') }}</div>
            <div class="text-xs text-muted">{{ data?.has_password ? $t('auth.account.passwordSet') : $t('auth.account.passwordNotSet') }}</div>
          </div>
        </li>
        <li v-for="m in methods" :key="m.provider" class="flex items-center gap-3 py-3">
          <span
            class="grid size-8 place-items-center rounded-full text-sm font-extrabold text-white"
            :class="{ 'bg-[#4285F4]': m.provider === 'google', 'bg-[#1877F2]': m.provider === 'facebook', 'bg-[#25D366]': m.provider === 'whatsapp' }"
            aria-hidden="true"
          >{{ m.provider === 'google' ? 'G' : m.provider === 'facebook' ? 'f' : 'W' }}</span>
          <div class="min-w-0 flex-1">
            <div class="font-semibold">{{ label(m.provider) }}</div>
            <div class="truncate text-xs text-muted">
              <template v-if="m.identity">{{ m.provider === 'whatsapp' ? data?.phone : m.identity.email || m.identity.name }} · {{ $t('auth.account.since', { date: fmtDate(m.identity.created_at) }) }}</template>
              <template v-else>{{ $t('auth.account.notConnected') }}</template>
            </div>
          </div>
          <UButton color="neutral" variant="soft" size="sm" type="submit" v-if="m.identity" :disabled="!!busy" @click="disconnect(m.provider)">{{ $t('auth.account.disconnect') }}</UButton>
          <UButton color="neutral" size="sm" type="submit" v-else-if="data?.providers[m.provider]" :disabled="!!busy" @click="connect(m.provider)">{{ busy === m.provider ? $t('auth.social.redirecting') : $t('auth.account.connect') }}</UButton>
        </li>
      </ul>
      <div v-if="waOpen" class="mt-4 rounded-2xl border border-line p-4">
        <AuthWhatsAppLogin link @done="waDone" @cancel="waOpen = false" />
      </div>
    </section>

    <AuthTwoFactorSettings class="mt-6" />

    <section class="card mt-6 p-5">
      <h2 class="font-bold">{{ data?.has_password ? $t('auth.account.changePassword') : $t('auth.account.setPassword') }}</h2>
      <p class="text-sm text-muted">{{ $t('auth.account.passwordHint') }}</p>
      <form class="mt-4 space-y-3" @submit.prevent="savePassword">
        <UInput v-if="data?.has_password" v-model="pw.current" type="password" :placeholder="$t('auth.account.currentPassword')" autocomplete="current-password" required />
        <UInput v-model="pw.password" type="password" minlength="8" :placeholder="$t('auth.account.newPassword')" autocomplete="new-password" required />
        <p v-if="pwMsg" class="text-sm text-mint-500">{{ pwMsg }}</p>
        <p v-if="pwErr" class="text-sm text-brand-700">{{ pwErr }}</p>
        <UButton type="submit">{{ $t('common.save') }}</UButton>
      </form>
    </section>
  </div>
</template>
