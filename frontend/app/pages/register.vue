<script setup lang="ts">
const { register } = useAuth()
const route = useRoute()
const form = reactive({ display_name: '', email: '', password: '' })
const captcha = ref('')
const human = ref<{ reset: () => void; pending: boolean } | null>(null)
const error = ref('')
const busy = ref(false)
async function submit() {
  error.value = ''
  busy.value = true
  try {
    await register(form.email, form.password, form.display_name, captcha.value || undefined)
    await navigateTo((route.query.next as string) || '/dashboard')
  } catch (e) {
    error.value = apiError(e)
    human.value?.reset() // the token was used up
  } finally {
    busy.value = false
  }
}
const { t } = useI18n()
useHead({ title: () => t('auth.register.title') + ' · zaokaiy' })
</script>

<template>
  <div class="mx-auto grid max-w-md px-4 py-16">
    <h1 class="page-title text-3xl">{{ $t('auth.register.heading') }}</h1>
    <p class="mt-1 text-muted">{{ $t('auth.register.intro') }}</p>
    <div class="card mt-8 space-y-4 p-6">
      <AuthSocialLogin :redirect="(route.query.next as string) || '/dashboard'" ask-name />
      <form class="space-y-4" @submit.prevent="submit">
      <div><label class="label">{{ $t('auth.register.displayName') }}</label><UInput v-model="form.display_name" required /></div>
      <div><label class="label">{{ $t('common.email') }}</label><UInput v-model="form.email" type="email" required autocomplete="email" /></div>
      <div>
        <label class="label">{{ $t('common.password') }}</label>
        <UInput v-model="form.password" type="password" required minlength="8" autocomplete="new-password" />
        <p class="mt-1 text-xs text-muted">{{ $t('auth.register.minChars') }}</p>
      </div>
      <AuthTurnstile ref="human" v-model="captcha" action="register" />
      <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
      <UButton type="submit" class="w-full" :disabled="busy || human?.pending">{{ busy ? $t('auth.register.submitting') : $t('auth.register.submit') }}</UButton>
      <p class="text-center text-sm text-muted">{{ $t('auth.register.haveAccount') }} <NuxtLink to="/login" class="font-semibold text-ink underline">{{ $t('auth.register.login') }}</NuxtLink></p>
      </form>
    </div>
  </div>
</template>
