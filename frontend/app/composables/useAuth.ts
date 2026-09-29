import type { LoginProvider, MfaChallenge, SessionResponse, SignInResponse, User } from '~/utils/types'

export function isMfaChallenge(r: SignInResponse): r is MfaChallenge {
  return (r as MfaChallenge).mfa_required === true
}

export function useAuth() {
  const token = useCookie<string | null>('zk_token', { maxAge: 60 * 60 * 24 * 7, sameSite: 'lax' })
  const user = useState<User | null>('auth:user', () => null)
  /** Sign-in waiting for a two-factor code (the /auth/two-factor page finishes it). */
  const mfa = useState<MfaChallenge | null>('auth:mfa', () => null)
  const api = useApi()

  const loggedIn = computed(() => !!token.value)

  async function fetchMe() {
    if (!token.value) {
      user.value = null
      return null
    }
    try {
      user.value = await api<User>('/auth/me')
    } catch {
      user.value = null
    }
    return user.value
  }

  function setSession(r: { token: string; user: User }) {
    token.value = r.token
    user.value = r.user
    mfa.value = null
  }

  /** A session is stored right away; a 2FA challenge is kept until the code is entered. */
  function accept<T extends SignInResponse>(r: T): T {
    if (isMfaChallenge(r)) mfa.value = r
    else setSession(r)
    return r
  }

  /** After signing in: go to `next`, or to the two-factor step first when the account needs it. */
  function continueTo(r: SignInResponse, next: string) {
    if (isMfaChallenge(r)) return navigateTo({ path: '/auth/two-factor', query: { next } }, { replace: true })
    return navigateTo(next, { replace: true })
  }

  /** `id` is an e-mail or a phone number in international format (+856…). `captcha` = Turnstile token. */
  async function login(id: string, password: string, captcha?: string) {
    return accept(await api<SignInResponse>('/auth/login', { method: 'POST', body: { email: id, password, captcha } }))
  }

  async function register(email: string, password: string, display_name: string, captcha?: string) {
    setSession(await api<{ token: string; user: User }>('/auth/register', { method: 'POST', body: { email, password, display_name, captcha } }))
  }

  /** Second step of a sign-in: authenticator or recovery code. */
  async function verifyTwoFactor(code: string) {
    if (!mfa.value) throw new Error('no sign-in in progress')
    const r = await api<SessionResponse>('/auth/2fa/verify', { method: 'POST', body: { mfa_token: mfa.value.mfa_token, code } })
    setSession(r)
    return r
  }

  /**
   * Google / Facebook: ask the API for the provider URL and leave the site. The provider sends the
   * browser back to /auth/callback. `link` connects the provider to the signed-in account instead.
   */
  async function startOAuth(provider: Exclude<LoginProvider, 'whatsapp'>, opts: { redirect?: string; link?: boolean } = {}) {
    const r = await api<{ url: string }>(`/auth/oauth/${provider}/start`, { method: 'POST', body: { redirect: opts.redirect || '/dashboard', link: !!opts.link } })
    if (import.meta.client) window.location.assign(r.url)
  }

  /** One-time code from /auth/callback → session (or a 2FA challenge). */
  async function exchange(code: string) {
    return accept(await api<SignInResponse>('/auth/exchange', { method: 'POST', body: { code } }))
  }

  async function whatsappSend(phone: string, lang: string, captcha?: string) {
    return api<{ sent: boolean; phone: string; expires_in: number; resend_in: number; dev_code?: string }>('/auth/whatsapp/send', { method: 'POST', body: { phone, lang, captcha } })
  }

  async function whatsappVerify(phone: string, code: string, opts: { display_name?: string; link?: boolean } = {}) {
    return accept(await api<SignInResponse>('/auth/whatsapp/verify', { method: 'POST', body: { phone, code, display_name: opts.display_name || undefined, link: !!opts.link } }))
  }

  function logout() {
    token.value = null
    user.value = null
    mfa.value = null
    useCookie('zk_shop').value = null
    navigateTo('/')
  }

  return { token, user, mfa, loggedIn, fetchMe, setSession, continueTo, login, register, verifyTwoFactor, startOAuth, exchange, whatsappSend, whatsappVerify, logout }
}
