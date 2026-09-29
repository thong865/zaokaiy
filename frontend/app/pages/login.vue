<script setup lang="ts">
const { login, continueTo } = useAuth()
const route = useRoute()
const form = reactive({ email: '', password: '' })
const captcha = ref('')
const human = ref<{ reset: () => void; pending: boolean } | null>(null)
const error = ref('')
const busy = ref(false)
async function submit() {
  error.value = ''
  busy.value = true
  try {
    const r = await login(form.email, form.password, captcha.value || undefined)
    await continueTo(r, (route.query.next as string) || '/dashboard')
  } catch (e) {
    error.value = apiError(e)
    human.value?.reset() // the token was used up
  } finally {
    busy.value = false
  }
}
const { t } = useI18n()
useHead({ title: () => t('auth.login.title') + ' · zaokaiy' })
</script>

<template>
  <div class="mx-auto grid max-w-md px-4 py-16">
    <h1 class="page-title text-3xl">{{ $t('auth.login.heading') }}</h1>
    <p class="mt-1 text-muted">{{ $t('auth.login.intro') }}</p>
    <div class="card mt-8 space-y-4 p-6">

      <form class="space-y-4" @submit.prevent="submit">
      <div><label class="label">{{ $t('auth.login.emailOrPhone') }}</label><UInput v-model="form.email" type="text" required autocomplete="username" :placeholder="$t('auth.login.emailOrPhonePlaceholder')" /></div>
      <div><label class="label">{{ $t('common.password') }}</label><UInput v-model="form.password" type="password" required autocomplete="current-password" /></div>
      <AuthTurnstile ref="human" v-model="captcha" action="login" />
      <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
      <UButton type="submit" class="w-full" :disabled="busy || human?.pending">{{ busy ? $t('auth.login.submitting') : $t('auth.login.submit') }}</UButton>
      <p class="text-center text-sm text-muted">{{ $t('auth.login.newHere') }} <NuxtLink to="/register" class="font-semibold text-ink underline">{{ $t('auth.login.createAccount') }}</NuxtLink></p>
      </form>
      <AuthSocialLogin :redirect="(route.query.next as string) || '/dashboard'" />
    </div>
  </div>
</template>
