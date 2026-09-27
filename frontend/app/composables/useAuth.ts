import type { User } from '~/utils/types'

export function useAuth() {
  const token = useCookie<string | null>('zk_token', { maxAge: 60 * 60 * 24 * 7, sameSite: 'lax' })
  const user = useState<User | null>('auth:user', () => null)
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

  async function login(email: string, password: string) {
    const r = await api<{ token: string; user: User }>('/auth/login', { method: 'POST', body: { email, password } })
    token.value = r.token
    user.value = r.user
  }

  async function register(email: string, password: string, display_name: string) {
    const r = await api<{ token: string; user: User }>('/auth/register', {
      method: 'POST',
      body: { email, password, display_name },
    })
    token.value = r.token
    user.value = r.user
  }

  function logout() {
    token.value = null
    user.value = null
    useCookie('zk_shop').value = null
    navigateTo('/')
  }

  return { token, user, loggedIn, fetchMe, login, register, logout }
}
