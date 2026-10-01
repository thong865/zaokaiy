/**
 * Browser preview (`npm run dev` opened in a normal browser, outside the desktop app): the Rust
 * commands are replaced by an in-memory demo so the UI can be looked at and styled without Tauri.
 * Inside the app `__TAURI_INTERNALS__` exists and this plugin does nothing.
 */
import type { Product, SaleRecord } from '~/utils/types'

export default defineNuxtPlugin(async () => {
  if ('__TAURI_INTERNALS__' in window) return
  const { mockIPC } = await import('@tauri-apps/api/mocks')

  const shop = {
    id: 'demo-shop', name: 'ຮ້ານດາວ Dao Market', currency: 'LAK', legal_name: 'Dao Market Sole Co., Ltd', tax_id: '123456789',
    branch: 'Head office', address: 'Ban Sisaket, Chanthabouly, Vientiane', phone: '020 5555 1234', vat_bps: 1000,
    prices_include_vat: true, receipt_prefix: 'R', receipt_footer: 'ຂອບໃຈ · Thank you!',
  }
  const device = { id: 'demo-device', code: 'T01', name: 'Front counter', last_seq: 41, cashier: { id: 'u1', name: 'Noy', can_void: true } }
  const cats = [
    { id: 'c1', parent_id: null, name: 'ເຄື່ອງດື່ມ Drinks', position: 1 },
    { id: 'c2', parent_id: null, name: 'ອາຫານ Food', position: 2 },
    { id: 'c3', parent_id: null, name: 'Household', position: 3 },
  ]
  const P = (i: number, name: string, price: number, stock: number, cat: string, barcode: string | null = null): Product => ({
    id: `p${i}`, name, sku: `SKU-${i}`, barcode, price_cents: price * 100, stock, category_id: cat, image: null,
  })
  const products: Product[] = [
    P(1, 'ເບຍລາວ Beerlao 640ml', 15000, 48, 'c1', '8850999508001'),
    P(2, 'ນ້ຳດື່ມ Tigerhead 1.5L', 6000, 120, 'c1'),
    P(3, 'ກາເຟລາວ Lao coffee', 25000, 12, 'c1'),
    P(4, 'Pepsi 330ml', 7000, 3, 'c1'),
    P(5, 'ເຂົ້າໜຽວ Sticky rice 1kg', 25000, 30, 'c2'),
    P(6, 'ແຈ່ວບອງ Jeow bong', 30000, 8, 'c2'),
    P(7, 'ໄສ້ອົ່ວ Sai oua sausage', 45000, 0, 'c2'),
    P(8, 'Mama noodles', 5000, 200, 'c2'),
    P(9, 'Dish soap 500ml', 18000, 16, 'c3'),
    P(10, 'Tissue box', 12000, 40, 'c3'),
    P(11, 'ໝາກພ້າວ Coconut', 10000, 25, 'c2'),
    P(12, 'Red Bull', 12000, 60, 'c1'),
  ]
  const sales: SaleRecord[] = []
  const bills = new Map<string, unknown>()
  const status = { paired: true, online: false, syncing: false, auth: 'ok', pending: 0, rejected: 0, products: products.length, last_sync_at: new Date(Date.now() - 7 * 60_000).toISOString(), last_error: null }
  let settings: unknown = { printer: { kind: 'network', host: '192.168.1.50', port: 9100, name: '', paper: 80, autoPrint: false, cut: true, drawer: true }, beep: true }
  let lang = 'en'
  const div = (a: number, b: number) => Math.floor((a * 2 + b) / (b * 2))

  mockIPC(
    (cmd, a) => {
      const args = (a ?? {}) as Record<string, any> // eslint-disable-line @typescript-eslint/no-explicit-any
      switch (cmd) {
        case 'app_info':
          status.pending = sales.filter((s) => s.sync_state === 'pending').length
          return { version: '0.1.0', os: 'browser', install_id: 'demo', api_base: 'https://your-domain.com/api', web_base: 'https://your-domain.com', lang, settings, shop, device, status }
        case 'set_lang':
          lang = args.lang
          return null
        case 'save_settings':
          settings = args.settings
          return null
        case 'search_products': {
          const q = String(args.q ?? '').toLowerCase()
          return products.filter((p) => (!q || p.name.toLowerCase().includes(q) || p.sku.toLowerCase().includes(q)) && (!args.categoryId || p.category_id === args.categoryId))
        }
        case 'scan_code':
          return products.find((p) => p.barcode === args.code || p.sku.toLowerCase() === String(args.code).toLowerCase()) ?? null
        case 'list_categories':
          return cats
        case 'list_bills':
          return [...bills.values()]
        case 'save_bill':
          bills.set(args.id, { id: args.id, label: args.label, data: args.data, created_at: new Date().toISOString() })
          return null
        case 'delete_bill':
          bills.delete(args.id)
          return null
        case 'create_sale': {
          const i = args.input
          const sub = i.lines.reduce((s: number, l: any) => s + l.unit_price_cents * l.qty - l.discount_cents, 0) // eslint-disable-line @typescript-eslint/no-explicit-any
          const net = sub - i.discount_cents
          const vat = div(net * shop.vat_bps, 10000 + shop.vat_bps)
          const paid = i.payments.reduce((s: number, p: any) => s + p.amount_cents, 0) // eslint-disable-line @typescript-eslint/no-explicit-any
          if (paid < net) throw `payment is short by ${net - paid}`
          const seq = device.last_seq + sales.length + 1
          const id = crypto.randomUUID()
          const created_at = new Date().toISOString()
          const rec: SaleRecord = {
            id, number: `RT01-${String(seq).padStart(6, '0')}`, seq, created_at, total_cents: net, status: 'completed', sync_state: 'pending',
            sync_error: null, synced_at: null, shortfall: [], void_reason: null, voided_at: null, void_sync: 'none', void_error: null,
            sale: {
              id, seq, number: `RT01-${String(seq).padStart(6, '0')}`, created_at, items: i.lines, discount_cents: i.discount_cents, vat_bps: shop.vat_bps,
              prices_include_vat: true, currency: 'LAK', subtotal_cents: sub, vat_cents: vat, total_cents: net, paid_cents: paid, change_cents: paid - net,
              payments: i.payments, note: i.note, customer: i.customer, print: { shop, device_code: 'T01', device_name: 'Front counter', cashier: 'Noy' },
            },
          }
          sales.unshift(rec)
          if (i.bill_id) bills.delete(i.bill_id)
          for (const l of i.lines) {
            const p = products.find((x) => x.id === l.product_id)
            if (p) p.stock -= l.qty
          }
          return rec
        }
        case 'list_sales':
          return sales.filter((s) => (!args.state || s.sync_state === args.state) && (!args.q || s.number.includes(args.q)))
        case 'get_sale':
          return sales.find((s) => s.id === args.id) ?? null
        case 'void_sale': {
          const s = sales.find((x) => x.id === args.id)!
          Object.assign(s, { status: 'voided', void_reason: args.reason, voided_at: new Date().toISOString(), void_sync: 'pending' })
          return s
        }
        case 'sales_summary': {
          const done = sales.filter((s) => s.status === 'completed')
          const methods: Record<string, number> = {}
          for (const s of done) for (const p of s.sale.payments) methods[p.method] = (methods[p.method] ?? 0) + p.amount_cents - (p.method === 'cash' ? s.sale.change_cents : 0)
          return { count: done.length, total_cents: done.reduce((a, s) => a + s.total_cents, 0), vat_cents: done.reduce((a, s) => a + s.sale.vat_cents, 0), voided: sales.length - done.length, voided_cents: 0, methods, top: [] }
        }
        case 'sync_now':
        case 'sync_status':
          return { ...status, pending: sales.filter((s) => s.sync_state === 'pending').length }
        case 'list_printers':
          return ['EPSON TM-T82', 'XP-80C']
        case 'retry_sale':
        case 'print_raster':
        case 'open_drawer':
        case 'unpair':
          return null
        case 'pair_start':
          return { code: 'K7Q4-9MZD', approve_url: 'https://your-domain.com/pos-pair?code=K7Q4-9MZD', expires_at: new Date(Date.now() + 600_000).toISOString(), interval: 3 }
        case 'pair_poll':
          return { status: 'pending' }
        default:
          return null
      }
    },
    { shouldMockEvents: true },
  )
})
