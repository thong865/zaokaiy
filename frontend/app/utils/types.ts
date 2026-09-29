export interface User {
  id: string
  /** Null for accounts created with WhatsApp or Facebook only. */
  email: string | null
  phone: string | null
  avatar_url: string | null
  display_name: string
  role: 'user' | 'admin'
  created_at: string
  /** Set while two-factor authentication is on. */
  totp_enabled_at: string | null
}

export type LoginProvider = 'google' | 'facebook' | 'whatsapp'
export interface AuthProviders { password: boolean; google: boolean; facebook: boolean; whatsapp: boolean; captcha_site_key: string | null }
/** Response of /auth/exchange and /auth/whatsapp/verify. */
export interface SessionResponse { token: string; user: User; is_new: boolean; linked: LoginProvider | null; redirect: string | null }
/** First factor passed; the account has 2FA on and needs a code (POST /auth/2fa/verify). */
export interface MfaChallenge { mfa_required: true; mfa_token: string; expires_in: number; redirect: string | null }
export type SignInResponse = SessionResponse | MfaChallenge
export interface TwoFactorStatus { enabled: boolean; enabled_at: string | null; recovery_codes_left: number }
export interface LoginIdentities {
  has_password: boolean
  email: string | null
  phone: string | null
  identities: { provider: LoginProvider; email: string | null; name: string; created_at: string; last_login_at: string }[]
  providers: AuthProviders
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
  vertical: 'general' | 'vehicle'
  entity_type: 'individual' | 'business'
  kyb_status: KybStatus
  kyb_verified_at: string | null
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
  cover?: ProductCover | null
}

/** A responsive WebP rendition of an uploaded image. */
export interface MediaVariant {
  w: number
  h: number
  url: string
}

/** First gallery image of a product, with renditions for srcset + a lazy-load placeholder. */
export interface ProductCover {
  url: string
  thumb_url?: string | null
  width?: number | null
  height?: number | null
  variants?: MediaVariant[]
  placeholder?: string
  color?: string
  alt?: string
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
  cover?: ProductCover | null
  shop_verified?: boolean
  vehicle?: VehicleSummary | null
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
  // delivery + cash on delivery
  carrier_code: string | null
  delivery_type: DeliveryType
  fee_payer: FeePayer
  shipping_fee_cents: number
  payment_method: 'prepaid' | 'cod'
  cod_fee_cents: number
  cod_amount_cents: number
  cod_status: CodStatus
  cod_remit_ref: string
  cod_collected_at: string | null
  cod_remitted_at: string | null
  tracking_no: string
  shipped_at: string | null
  /** items + shipping (when charged at checkout) + COD fee */
  grand_total_cents: number
}

export type DeliveryType = 'branch' | 'home'
/** buyer: fee added at checkout · destination: buyer pays the courier on pickup (ປາຍທາງ) · seller: free shipping */
export type FeePayer = 'buyer' | 'destination' | 'seller'
export type CodStatus = 'none' | 'pending' | 'collected' | 'remitted' | 'returned'

export interface Carrier {
  code: string
  name: string
  name_lo: string
  website: string
  /** "{tracking}" is replaced by the tracking number; empty = no tracking page */
  tracking_url: string
  phone: string
  supports_cod: boolean
  active: boolean
  sort: number
}

/** A shop's terms for one courier (GET /shops/:id/shipping, /shipping/options, social shipping_options). */
export interface ShippingOption {
  carrier_code: string
  name: string
  name_lo: string
  website: string
  tracking_url: string
  supports_cod: boolean
  enabled: boolean
  fee_payer: FeePayer
  fee_cents: number
  /** null = no home delivery (branch pickup only) */
  home_fee_cents: number | null
  free_over_cents: number | null
  cod_enabled: boolean
  cod_fee_cents: number
  eta: string
  note: string
  configured: boolean
}

export interface CodRow {
  kind: 'order' | 'social'
  id: string
  number: string
  customer: string
  phone: string
  carrier_code: string | null
  tracking_no: string
  tracking_link: string | null
  cod_amount_cents: number
  cod_status: CodStatus
  cod_remit_ref: string
  order_status: string
  currency: string
  created_at: string
  shipped_at: string | null
  cod_collected_at: string | null
  cod_remitted_at: string | null
}
export interface CodLedger {
  rows: CodRow[]
  totals: { carrier_code: string | null; cod_status: CodStatus; count: number; amount_cents: number }[]
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
  /** WebP renditions, largest first (empty for videos, GIFs and external URLs). */
  variants?: MediaVariant[]
  placeholder?: string
  dominant_color?: string
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
  variants?: MediaVariant[]
  placeholder?: string
  color?: string
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
  carrier_code: string | null
  delivery_type: DeliveryType
  fee_payer: FeePayer
  cod_fee_cents: number
  cod_amount_cents: number
  cod_status: CodStatus
  cod_remit_ref: string
  cod_collected_at: string | null
  cod_remitted_at: string | null
  shipped_at: string | null
}

export interface SocialOrderDoc {
  order: SocialOrder
  items: SocialOrderItem[]
  customer: { name: string; provider: SocialProvider; external_user_id?: string }
  shop: DocShop & { payment_instructions: string; slug: string }
  checkout_url?: string
  comments?: { message: string; result: string; created_at: string }[]
  /** Public checkout only: the shop's enabled couriers. */
  shipping_options?: ShippingOption[]
}

/** Key vehicle facts attached to catalog products. */
export interface VehicleSummary {
  vehicle_type: string
  make: string
  model: string
  year: number
  mileage_km: number
  fuel: string
  transmission: string
  condition: string
  buy_online: boolean
  sale_status: 'available' | 'reserved' | 'sold'
}

/** Spec sheet. Seller view has plate_no/vin; the public view has vin_last4 instead. */
export interface VehicleSpec {
  product_id?: string
  vehicle_type: string
  condition: string
  make: string
  model: string
  variant: string
  year: number
  mileage_km: number
  fuel: string
  transmission: string
  body_type: string
  drive: string
  engine_cc: number | null
  power_hp: number | null
  seats: number | null
  doors: number | null
  color: string
  owners: number | null
  registered: boolean
  plate_province: string
  plate_no?: string
  vin?: string
  vin_last4?: string | null
  location: string
  features: string[]
  warranty_months: number | null
  deposit_cents: number
  negotiable: boolean
  finance_available: boolean
  buy_online: boolean
  sale_status: 'available' | 'reserved' | 'sold'
  reserved_until?: string | null
  sold_at?: string | null
}

export interface VehicleCard {
  id: string
  shop_id: string
  name: string
  price_cents: number
  currency: string
  shop_slug: string
  shop_name: string
  images: string[]
  cover?: ProductCover | null
  stock: number
  via_shop_id: string | null
  created_at: string
  vehicle_type: string
  condition: string
  make: string
  model: string
  variant: string
  year: number
  mileage_km: number
  fuel: string
  transmission: string
  body_type: string
  engine_cc: number | null
  color: string
  location: string
  negotiable: boolean
  finance_available: boolean
  buy_online: boolean
  deposit_cents: number
  sale_status: 'available' | 'reserved' | 'sold'
}

export interface VehicleFacets {
  types: [string, number][]
  makes: { make: string; count: number; models: { model: string; count: number }[] }[]
  fuel: [string, number][]
  transmission: [string, number][]
  condition: [string, number][]
  body_type: [string, number][]
  year: { min: number | null; max: number | null }
  price: { min: number | null; max: number | null }
  mileage: { max: number | null }
}

export interface VehicleSearch {
  shop: { id: string; slug: string; name: string; description: string; phone: string; address: string; currency: string; logo_url: string | null; vertical: string } | null
  items: VehicleCard[]
  total: number
  facets: VehicleFacets
}

export interface ShopContact {
  name: string
  phone: string
  address: string
}

export interface VehicleLead {
  id: string
  shop_id: string
  product_id: string
  kind: 'enquiry' | 'test_drive' | 'offer' | 'reserve' | 'finance'
  name: string
  phone: string
  message: string
  preferred_at: string | null
  offer_cents: number | null
  deposit_cents: number
  status: 'new' | 'contacted' | 'scheduled' | 'won' | 'lost'
  scheduled_at: string | null
  deposit_paid_at: string | null
  seller_note: string
  created_at: string
  updated_at: string
  // joined
  product_name?: string
  price_cents?: number
  currency?: string
  cover?: ProductCover | null
  image?: string | null
  make?: string
  model?: string
  year?: number
  sale_status?: 'available' | 'reserved' | 'sold'
  vehicle_deposit_cents?: number
}

/** Vehicle spec form state (numbers are strings while typing). */
export interface VehicleDraft {
  vehicle_type: string
  condition: string
  make: string
  model: string
  variant: string
  year: string | number
  mileage_km: string | number
  fuel: string
  transmission: string
  body_type: string
  drive: string
  engine_cc: string | number
  power_hp: string | number
  seats: string | number
  doors: string | number
  color: string
  owners: string | number
  registered: boolean
  plate_province: string
  plate_no: string
  vin: string
  location: string
  features: string[]
  warranty_months: string | number
  deposit: string | number
  negotiable: boolean
  finance_available: boolean
  buy_online: boolean
  sale_status: string
}

// ---- corporate KYC (business verification)
export type KybStatus = 'none' | 'draft' | 'submitted' | 'changes_requested' | 'approved' | 'rejected' | 'revoked'

export interface KybProfile {
  company_type: string
  legal_name: string
  legal_name_local: string
  registration_no: string
  registration_date: string | null
  tax_id: string
  country: string
  province: string
  registered_address: string
  business_activity: string
  website: string
  contact_email: string
  contact_phone: string
  bank_name: string
  bank_account_name: string
  bank_account_last4: string
  /** Admin view only (decrypted). */
  bank_account_number?: string | null
  risk_flags: string[]
  submitted_at: string | null
  reviewed_at: string | null
  review_note: string
  review_due_at: string | null
  updated_at: string
}

export interface KybPerson {
  id: string
  roles: string[]
  full_name: string
  title: string
  nationality: string
  date_of_birth: string | null
  id_type: string
  id_last4: string
  has_id_number: boolean
  /** Admin view only (decrypted). */
  id_number?: string | null
  ownership_bps: number
  is_pep: boolean
}

export interface KybDocument {
  id: string
  kind: string
  content_type: string
  size_bytes: number
  sha256: string
  original_name: string
  expires_on: string | null
  status: 'pending' | 'accepted' | 'rejected'
  review_note: string
  created_at: string
}

export interface KybView {
  shop: { id: string; name: string; slug: string; entity_type: 'individual' | 'business'; kyb_status: KybStatus; kyb_verified_at: string | null; owner_id: string }
  profile: KybProfile | null
  persons: KybPerson[]
  documents: KybDocument[]
  missing: string[]
  editable: boolean
  events: { action: string; note: string; created_at: string; actor: string; by_admin: boolean }[]
  required_to_publish: boolean
  options: { company_types: string[]; shareholder_types: string[]; doc_kinds: string[]; id_types: string[]; roles: string[]; max_doc_mb: number }
  owner?: { display_name: string; email: string | null; phone: string | null } | null
}
