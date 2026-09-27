export interface User {
  id: string
  email: string
  display_name: string
  role: 'user' | 'admin'
  created_at: string
}

export interface Shop {
  id: string
  owner_id: string
  slug: string
  name: string
  description: string
  logo_url: string | null
  currency: string
  kind: 'seller' | 'creator'
  created_at: string
  auto_approve: boolean
  legal_name: string
  tax_id: string
  branch: string
  address: string
  phone: string
  vat_bps: number
  prices_include_vat: boolean
  receipt_prefix: string
  invoice_prefix: string
  receipt_footer: string
}

export interface Product {
  id: string
  shop_id: string
  sku: string
  name: string
  description: string
  price_cents: number
  images: string[]
  category: string
  status: 'draft' | 'active' | 'archived'
  stock: number
  low_stock_threshold: number
  allow_resell: boolean
  commission_bps: number
  created_at: string
  updated_at: string
  category_id: string | null
  shop_category_id: string | null
  review_status: ReviewStatus
  review_note: string
  submitted_at: string | null
  reviewed_at: string | null
  barcode: string | null
  social_code: string | null
}

export type ReviewStatus = 'not_submitted' | 'pending' | 'approved' | 'rejected'

export interface CatalogProduct {
  id: string
  shop_id: string
  sku: string
  name: string
  description: string
  price_cents: number
  images: string[]
  category: string
  stock: number
  allow_resell: boolean
  commission_bps: number
  category_id: string | null
  shop_category_id: string | null
  shop_slug: string
  shop_name: string
  currency: string
  via_shop_id: string | null
}

export interface CartLine {
  product_id: string
  via_shop_id: string | null
  name: string
  price_cents: number
  currency: string
  image: string | null
  shop_name: string
  qty: number
}

export interface OrderItem {
  id: string
  order_id: string
  product_id: string
  product_name: string
  supplier_shop_id: string
  seller_shop_id: string | null
  qty: number
  unit_price_cents: number
  commission_bps: number
  commission_cents: number
}

export interface Order {
  id: string
  buyer_id: string
  status: 'pending' | 'paid' | 'shipped' | 'completed' | 'cancelled'
  total_cents: number
  currency: string
  shipping_address: Record<string, string>
  created_at: string
  items: OrderItem[]
}

export interface Content {
  id: string
  shop_id: string
  product_id: string | null
  kind: 'post' | 'caption' | 'video_script' | 'description' | 'ad_copy'
  title: string
  body: string
  media_url: string | null
  ai_generated: boolean
  status: 'draft' | 'published'
  created_at: string
}

export interface AdCampaign {
  id: string
  shop_id: string
  product_id: string
  headline: string
  body: string
  image_url: string | null
  budget_cents: number
  cpc_cents: number
  spent_cents: number
  impressions: number
  clicks: number
  status: 'draft' | 'active' | 'paused' | 'ended'
  created_at: string
}

export interface Partnership {
  id: string
  supplier_shop_id: string
  supplier_name: string
  reseller_shop_id: string
  reseller_name: string
  status: 'pending' | 'approved' | 'rejected' | 'revoked'
  commission_bps: number | null
  message: string
  created_at: string
}

export interface MediaAsset {
  id: string
  shop_id: string
  kind: 'image' | 'video'
  source: 'upload' | 'url'
  mime: string
  url: string
  thumb_url: string | null
  size_bytes: number
  width: number | null
  height: number | null
  alt: string
  original_name: string
  created_at: string
  product_count?: number
  position?: number
}

export interface PublicMedia {
  id: string
  kind: 'image' | 'video'
  url: string
  thumb_url: string | null
  alt: string
  mime: string
  width: number | null
  height: number | null
}

export interface CategoryNode {
  id: string
  shop_id: string | null
  parent_id: string | null
  name: string
  slug: string
  description: string
  image_url: string | null
  position: number
  active: boolean
  depth: number
  path: string
  product_count: number
  total_count: number
  child_count: number
}

export interface ReviewEvent {
  action: 'submitted' | 'approved' | 'rejected' | 'auto_approved' | 'resubmitted'
  note: string
  created_at: string
  actor: string | null
}

export type PayMethod = 'cash' | 'card' | 'transfer' | 'qr' | 'other'

export interface PosPayment {
  method: PayMethod
  amount_cents: number
  reference?: string
}

export interface PosSale {
  id: string
  shop_id: string
  number: string
  status: 'completed' | 'voided'
  subtotal_cents: number
  discount_cents: number
  vat_bps: number
  prices_include_vat: boolean
  vat_cents: number
  total_cents: number
  paid_cents: number
  change_cents: number
  payments: PosPayment[]
  currency: string
  note: string
  invoice_number: string | null
  invoice_issued_at: string | null
  customer_name: string
  customer_tax_id: string
  customer_branch: string
  customer_address: string
  customer_phone: string
  void_reason: string
  voided_at: string | null
  created_at: string
}

export interface PosItem {
  id: string
  product_id: string | null
  name: string
  sku: string
  qty: number
  unit_price_cents: number
  discount_cents: number
  line_total_cents: number
}

export interface DocShop {
  id?: string
  name: string
  slug?: string
  logo_url: string | null
  legal_name: string
  tax_id: string
  branch: string
  address: string
  phone: string
  currency: string
  receipt_footer: string
}

export interface PosDoc {
  sale: PosSale
  items: PosItem[]
  cashier: string | null
  amount_before_vat_cents: number
  shop: DocShop
}

export type SocialProvider = 'facebook' | 'tiktok' | 'whatsapp' | 'webhook'

export interface SocialFeedItem {
  id: string
  provider: SocialProvider
  kind: string
  user_name: string
  user_id: string
  message: string
  parsed: { code: string; qty: number }[]
  result: 'ignored' | 'claimed' | 'partial' | 'sold_out' | 'blocked' | 'error'
  order_number: string | null
  reply_text: string
  reply_status: string
  reply_error: string
  created_at: string
}

export interface SocialOrderItem { id: string; product_id: string | null; code: string; name: string; qty: number; unit_price_cents: number }

export interface SocialOrder {
  id: string
  number: string
  status: 'open' | 'confirmed' | 'paid' | 'shipped' | 'completed' | 'cancelled' | 'expired'
  subtotal_cents: number
  shipping_cents: number
  total_cents: number
  currency: string
  ship_name: string
  ship_phone: string
  ship_address: string
  customer_note: string
  payment_method: string
  payment_ref: string
  tracking_no: string
  expires_at: string
  confirmed_at: string | null
  paid_at: string | null
  created_at: string
}

export interface SocialOrderDoc {
  order: SocialOrder
  items: SocialOrderItem[]
  customer: { name: string; provider: SocialProvider; external_user_id?: string }
  shop: DocShop & { payment_instructions: string; slug: string }
  checkout_url?: string
  comments?: { message: string; result: string; created_at: string }[]
}
