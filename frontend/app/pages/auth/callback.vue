<script setup lang="ts">
/** Landing page after Google / Facebook: swaps the one-time code for a session, or explains the error. */
const route = useRoute()
const { t } = useI18n()
const { exchange, continueTo } = useAuth()
useHead({ title: () => t('auth.callback.title') + ' · zaokaiy', meta: [{ name: 'robots', content: 'noindex' }] })

const KNOWN = ['cancelled', 'expired', 'provider', 'email_exists', 'already_linked', 'server'] as const
const errorCode = ref<string>((route.query.error as string) || '')
const provider = computed(() => {
  const p = route.query.provider as string
  return p === 'google' ? 'Google' : p === 'facebook' ? 'Facebook' : ''
})
const message = ref('')

onMounted(async () => {
  const code = route.query.code as string | undefined
  if (errorCode.value || !code) {
    if (!errorCode.value) errorCode.value = 'expired'
    return
  }
  try {
    const r = await exchange(code)
    const next = 'linked' in r && r.linked ? `/account?linked=${r.linked}` : r.redirect || '/dashboard'
    await continueTo(r, next)
  } catch (e) {
    errorCode.value = 'exchange'
    message.value = apiError(e)
  }
})
const errorText = computed(() => {
  if (message.value) return message.value
  const k = (KNOWN as readonly string[]).includes(errorCode.value) ? errorCode.value : 'server'
  return t(`auth.callback.errors.${k}`, { provider: provider.value })
})
</script>

<template>
  <div class="mx-auto grid max-w-md px-4 py-20 text-center">
    <template v-if="!errorCode">
      <div class="mx-auto size-10 animate-spin rounded-full border-4 border-line border-t-brand-500" aria-hidden="true" />
      <p class="mt-4 font-semibold">{{ $t('auth.callback.signingIn') }}</p>
    </template>
    <div v-else class="card p-6">
      <h1 class="text-xl font-extrabold">{{ $t('auth.callback.failedTitle') }}</h1>
      <p class="mt-2 text-sm text-muted">{{ errorText }}</p>
      <div class="mt-5 flex justify-center gap-2">
        <UButton v-if="errorCode === 'already_linked'" to="/account">{{ $t('common.nav.account') }}</UButton>
        <UButton v-else to="/login">{{ $t('auth.callback.backToLogin') }}</UButton>
      </div>
    </div>
  </div>
</template>
