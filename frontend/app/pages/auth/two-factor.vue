<script setup lang="ts">
/** Second sign-in step for accounts with two-factor authentication: authenticator or recovery code. */
const route = useRoute()
const { t } = useI18n()
const { mfa, verifyTwoFactor } = useAuth()
useHead({ title: () => t('auth.twoFactor.title') + ' · zaokaiy', meta: [{ name: 'robots', content: 'noindex' }] })

const code = ref('')
const recovery = ref(false)
const busy = ref(false)
const error = ref('')
const next = computed(() => {
  const n = (route.query.next as string) || mfa.value?.redirect || '/dashboard'
  return n.startsWith('/') && !n.startsWith('//') ? n : '/dashboard'
})

// Reloading the page loses the pending sign-in (it only lives in memory): start over.
onMounted(() => {
  if (!mfa.value) navigateTo({ path: '/login', query: { next: next.value } }, { replace: true })
})

async function submit() {
  if (busy.value) return
  error.value = ''
  busy.value = true
  try {
    await verifyTwoFactor(code.value)
    await navigateTo(next.value, { replace: true })
  } catch (e) {
    error.value = apiError(e)
    code.value = ''
  } finally {
    busy.value = false
  }
}
function toggle() {
  recovery.value = !recovery.value
  code.value = error.value = ''
}

// Authenticator codes submit themselves once 6 digits are in.
watch(code, (v) => {
  if (recovery.value) return
  const d = v.replace(/\D/g, '').slice(0, 6)
  if (d !== v) code.value = d
  else if (d.length === 6) submit()
})
</script>

<template>
  <div class="mx-auto grid max-w-md px-4 py-16">
    <h1 class="page-title text-3xl">{{ $t('auth.twoFactor.heading') }}</h1>
    <p class="mt-1 text-muted">{{ recovery ? $t('auth.twoFactor.introRecovery') : $t('auth.twoFactor.intro') }}</p>
    <form class="card mt-8 space-y-4 p-6" @submit.prevent="submit">
      <UInput
        v-if="!recovery"
        v-model="code"
        class="text-center font-mono text-2xl tracking-[0.5em]"
        inputmode="numeric"
        autocomplete="one-time-code"
        maxlength="6"
        placeholder="••••••"
        :aria-label="$t('auth.twoFactor.code')"
        autofocus
        required
      />
      <UInput v-else v-model="code" class="font-mono" autocomplete="off" autocapitalize="off" spellcheck="false" placeholder="xxxxx-xxxxx" :aria-label="$t('auth.twoFactor.recoveryCode')" autofocus required />
      <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
      <UButton type="submit" class="w-full" :disabled="busy || (!recovery && code.length !== 6) || (recovery && code.trim().length < 10)">
        {{ busy ? $t('auth.twoFactor.verifying') : $t('auth.twoFactor.verify') }}
      </UButton>
      <div class="flex justify-between text-xs">
        <button type="button" class="font-semibold underline" @click="toggle">{{ recovery ? $t('auth.twoFactor.useApp') : $t('auth.twoFactor.useRecovery') }}</button>
        <NuxtLink to="/login" class="text-muted underline hover:text-ink">{{ $t('auth.twoFactor.cancel') }}</NuxtLink>
      </div>
    </form>
  </div>
</template>
