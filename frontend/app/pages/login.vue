<script setup lang="ts">
const { login } = useAuth()
const route = useRoute()
const form = reactive({ email: '', password: '' })
const error = ref('')
const busy = ref(false)
async function submit() {
  error.value = ''
  busy.value = true
  try {
    await login(form.email, form.password)
    await navigateTo((route.query.next as string) || '/dashboard')
  } catch (e) {
    error.value = apiError(e)
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
    <form class="card mt-8 space-y-4 p-6" @submit.prevent="submit">
      <div><label class="label">{{ $t('common.email') }}</label><input v-model="form.email" type="email" required class="input" autocomplete="email" ></div>
      <div><label class="label">{{ $t('common.password') }}</label><input v-model="form.password" type="password" required class="input" autocomplete="current-password" ></div>
      <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
      <button class="btn-primary w-full" :disabled="busy">{{ busy ? $t('auth.login.submitting') : $t('auth.login.submit') }}</button>
      <p class="text-center text-sm text-muted">{{ $t('auth.login.newHere') }} <NuxtLink to="/register" class="font-semibold text-ink underline">{{ $t('auth.login.createAccount') }}</NuxtLink></p>
    </form>
  </div>
</template>
