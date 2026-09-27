<script setup lang="ts">
const { register } = useAuth()
const route = useRoute()
const form = reactive({ display_name: '', email: '', password: '' })
const error = ref('')
const busy = ref(false)
async function submit() {
  error.value = ''
  busy.value = true
  try {
    await register(form.email, form.password, form.display_name)
    await navigateTo((route.query.next as string) || '/dashboard')
  } catch (e) {
    error.value = apiError(e)
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
    <form class="card mt-8 space-y-4 p-6" @submit.prevent="submit">
      <div><label class="label">{{ $t('auth.register.displayName') }}</label><input v-model="form.display_name" required class="input" ></div>
      <div><label class="label">{{ $t('common.email') }}</label><input v-model="form.email" type="email" required class="input" autocomplete="email" ></div>
      <div>
        <label class="label">{{ $t('common.password') }}</label>
        <input v-model="form.password" type="password" required minlength="8" class="input" autocomplete="new-password" >
        <p class="mt-1 text-xs text-muted">{{ $t('auth.register.minChars') }}</p>
      </div>
      <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
      <button class="btn-primary w-full" :disabled="busy">{{ busy ? $t('auth.register.submitting') : $t('auth.register.submit') }}</button>
      <p class="text-center text-sm text-muted">{{ $t('auth.register.haveAccount') }} <NuxtLink to="/login" class="font-semibold text-ink underline">{{ $t('auth.register.login') }}</NuxtLink></p>
    </form>
  </div>
</template>
