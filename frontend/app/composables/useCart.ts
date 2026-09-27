import type { CartLine } from '~/utils/types'

const KEY = 'zk_cart_v1'

/** Client-side cart persisted in localStorage. Lines are keyed by product + reseller storefront. */
export function useCart() {
  const lines = useState<CartLine[]>('cart', () => [])
  const loaded = useState('cart:loaded', () => false)

  if (import.meta.client && !loaded.value) {
    try {
      lines.value = JSON.parse(localStorage.getItem(KEY) || '[]')
    } catch {
      lines.value = []
    }
    loaded.value = true
    watch(lines, (v) => {
      try {
        localStorage.setItem(KEY, JSON.stringify(v))
      } catch {}
    }, { deep: true })
  }

  const count = computed(() => lines.value.reduce((n, l) => n + l.qty, 0))
  const total = computed(() => lines.value.reduce((n, l) => n + l.qty * l.price_cents, 0))

  function add(line: Omit<CartLine, 'qty'>, qty = 1) {
    const existing = lines.value.find((l) => l.product_id === line.product_id && l.via_shop_id === line.via_shop_id)
    if (existing) existing.qty = Math.min(existing.qty + qty, 100)
    else lines.value.push({ ...line, qty })
  }

  function setQty(i: number, qty: number) {
    const line = lines.value[i]
    if (!line) return
    if (qty <= 0) lines.value.splice(i, 1)
    else line.qty = Math.min(qty, 100)
  }

  function clear() {
    lines.value = []
  }

  return { lines, count, total, add, setQty, clear }
}
