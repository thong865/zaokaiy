export interface Shop {
  id: string
  name: string
  currency: string
  legal_name: string
  tax_id: string
  branch: string
  address: string
  phone: string
  vat_bps: number
  prices_include_vat: boolean
  receipt_prefix: string
  receipt_footer: string
  logo_url?: string | null
}

export interface Device {
  id: string
  code: string
  name: string
  last_seq: number
  cashier: { id: string; name: string | null; can_void: boolean }
}

export interface SyncStatus {
  paired: boolean
  online: boolean
  syncing: boolean
  auth: 'ok' | 'unpaired' | 'forbidden' | string
  pending: number
  rejected: number
  products: number
  last_sync_at: string | null
  last_error: string | null
}

export interface PrinterSettings {
  kind: 'none' | 'network' | 'system' | 'window'
  host: string
  port: number
  name: string
  paper: 58 | 80
  autoPrint: boolean
  cut: boolean
  drawer: boolean
}

export interface Settings {
  printer: PrinterSettings
  beep: boolean
}

export interface AppInfo {
  version: string
  os: string
  install_id: string
  api_base: string | null
  web_base: string | null
  lang: 'en' | 'lo'
  settings: Partial<Settings> | null
  shop: Shop | null
  device: Device | null
  status: SyncStatus
}

export interface Product {
  id: string
  name: string
  sku: string
  barcode: string | null
  price_cents: number
  stock: number
  category_id: string | null
  image: string | null
}

export interface Category {
  id: string
  parent_id: string | null
  name: string
  position: number
}

export interface Line {
  key: string
  product_id: string | null
  name: string
  sku: string
  qty: number
  unit_price_cents: number
  discount_cents: number
}

export interface Payment {
  method: 'cash' | 'card' | 'transfer' | 'qr' | 'other'
  amount_cents: number
  reference: string
}

export interface BillData {
  lines: Line[]
  discount: { mode: 'amount' | 'percent'; value: number }
  note: string
  customer: { name: string; phone: string }
}

export interface Bill {
  id: string
  label: string
  data: BillData
  created_at: string
}

export interface SaleDoc {
  id: string
  seq: number
  number: string
  created_at: string
  items: Omit<Line, 'key'>[]
  discount_cents: number
  vat_bps: number
  prices_include_vat: boolean
  currency: string
  subtotal_cents: number
  vat_cents: number
  total_cents: number
  paid_cents: number
  change_cents: number
  payments: Payment[]
  note: string
  customer: { name?: string; phone?: string } | null
  print: { shop: Shop; device_code: string; device_name: string; cashier: string | null } | null
}

export interface SaleRecord {
  id: string
  number: string
  seq: number
  created_at: string
  total_cents: number
  status: 'completed' | 'voided'
  sync_state: 'pending' | 'synced' | 'rejected'
  sync_error: string | null
  synced_at: string | null
  shortfall: { product_id: string; name: string; qty: number }[]
  void_reason: string | null
  voided_at: string | null
  void_sync: 'none' | 'pending' | 'synced' | 'rejected'
  void_error: string | null
  sale: SaleDoc
}

export const DEFAULT_SETTINGS: Settings = {
  printer: { kind: 'none', host: '', port: 9100, name: '', paper: 80, autoPrint: true, cut: true, drawer: true },
  beep: true,
}
