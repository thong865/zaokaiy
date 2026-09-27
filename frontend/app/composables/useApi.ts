import type { FetchError } from 'ofetch'

/** Typed fetch wrapper for the Rust API. Attaches the JWT and normalises errors. */
export function useApi() {
  const config = useRuntimeConfig()
  const token = useCookie<string | null>('zk_token', { maxAge: 60 * 60 * 24 * 7, sameSite: 'lax' })
  const nuxt = useNuxtApp()
  const baseURL = (import.meta.server && (config.apiBaseServer as string)) || config.public.apiBase

  return $fetch.create({
    baseURL,
    onRequest({ options }) {
      const h = new Headers(options.headers as HeadersInit)
      if (token.value) h.set('Authorization', `Bearer ${token.value}`)
      // The API translates its error messages for Lao.
      h.set('Accept-Language', (nuxt.$i18n as { locale: { value: string } }).locale.value)
      options.headers = h
    },
    onResponseError({ response }) {
      if (response.status === 401 && token.value) {
        token.value = null
      }
    },
  })
}

export function apiError(e: unknown): string {
  const fe = e as FetchError<{ message?: string }>
  return fe?.data?.message || fe?.message || tr('common.errors.generic')
}
