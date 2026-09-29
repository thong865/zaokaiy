/** Fetch a private KYC document with the session token and show it via a temporary blob URL. */
export function useKybFile() {
  const api = useApi()
  const url = ref<string | null>(null)
  const type = ref('')
  const loading = ref(false)
  const error = ref('')
  function close() {
    if (url.value) URL.revokeObjectURL(url.value)
    url.value = null
  }
  async function open(id: string) {
    close()
    loading.value = true
    error.value = ''
    try {
      const blob = await api<Blob>(`/kyb/documents/${id}/file`, { responseType: 'blob' })
      type.value = blob.type
      url.value = URL.createObjectURL(blob)
    } catch (e) {
      error.value = apiError(e)
    } finally {
      loading.value = false
    }
  }
  onBeforeUnmount(close)
  return { url, type, loading, error, open, close }
}
