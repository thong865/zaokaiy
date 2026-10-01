import type { ApiCore } from '~/composables/useCores'

/** Which cores the API runs (fetched once during SSR, reused on the client). */
export default defineNuxtPlugin(async () => {
  const enabled = useState<ApiCore[] | null>('zk-cores', () => null)
  if (enabled.value) return
  const api = useApi()
  enabled.value = await api<{ cores: ApiCore[] }>('/cores')
    .then((r) => r.cores)
    .catch(() => null) // API down or older API: show every core the front end has
})
