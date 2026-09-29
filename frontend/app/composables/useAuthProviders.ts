import type { AuthProviders } from '~/utils/types'

/** Enabled login methods and the Turnstile site key (null = no human check), from the API. */
export function useAuthProviders() {
  const api = useApi()
  return useAsyncData('auth-providers', () => api<AuthProviders>('/auth/providers'), {
    default: (): AuthProviders => ({ password: true, google: false, facebook: false, whatsapp: false, captcha_site_key: null }),
  })
}
