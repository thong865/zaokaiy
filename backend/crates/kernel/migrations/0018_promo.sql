-- Module `promo`: promotion campaigns, coupons and per-shop loyalty points.
-- Campaigns are run by the platform (shop_id IS NULL, e.g. "first sign-up with Facebook") or by a shop.
-- Every coupon use and every points change is written once; cancelling an order reverses them.

-- Discounts on the three kinds of sales. platform_discount_cents = the part the platform pays for
-- (platform campaigns): the shop is owed it, and it still counts as the shop's taxable sale.
ALTER TABLE orders ADD COLUMN discount_cents BIGINT NOT NULL DEFAULT 0 CHECK (discount_cents >= 0);
ALTER TABLE orders ADD COLUMN platform_discount_cents BIGINT NOT NULL DEFAULT 0 CHECK (platform_discount_cents >= 0);
-- grand_total now takes the discount off (generated columns can't be altered in PG16: re-create).
ALTER TABLE orders DROP COLUMN grand_total_cents;
ALTER TABLE orders ADD COLUMN grand_total_cents BIGINT GENERATED ALWAYS AS
    (total_cents - discount_cents + CASE WHEN fee_payer = 'buyer' THEN shipping_fee_cents ELSE 0 END + cod_fee_cents) STORED;
ALTER TABLE social_orders ADD COLUMN discount_cents BIGINT NOT NULL DEFAULT 0 CHECK (discount_cents >= 0);
ALTER TABLE social_orders ADD COLUMN platform_discount_cents BIGINT NOT NULL DEFAULT 0 CHECK (platform_discount_cents >= 0);
ALTER TABLE pos_sales ADD COLUMN platform_discount_cents BIGINT NOT NULL DEFAULT 0 CHECK (platform_discount_cents >= 0);
ALTER TABLE pos_sales ADD COLUMN member_phone TEXT NOT NULL DEFAULT '';

CREATE TABLE promo_campaigns (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id            UUID REFERENCES shops(id) ON DELETE CASCADE,       -- NULL = platform campaign
    name               TEXT NOT NULL,
    description        TEXT NOT NULL DEFAULT '',
    -- signup      : platform only — new accounts that signed up with one of `providers`
    -- first_order : shop only — after a customer's first completed order at the shop
    -- code        : anyone who types `code` at checkout (once per customer)
    trigger            TEXT NOT NULL CHECK (trigger IN ('signup','first_order','code')),
    providers          TEXT[] NOT NULL DEFAULT '{}',                      -- signup: google|facebook|whatsapp|email; empty = any
    code               TEXT,                                               -- trigger = code (upper case)
    -- amount (value = cents) · percent (value = basis points, 1000 = 10%) · free_shipping · points (value = points; shop only)
    reward_type        TEXT NOT NULL CHECK (reward_type IN ('amount','percent','free_shipping','points')),
    reward_value       BIGINT NOT NULL DEFAULT 0 CHECK (reward_value >= 0),
    max_discount_cents BIGINT NOT NULL DEFAULT 0,                          -- percent cap; 0 = no cap
    min_spend_cents    BIGINT NOT NULL DEFAULT 0,
    currency           TEXT NOT NULL DEFAULT 'LAK',                        -- platform campaigns: amounts are in this currency
    coupon_days        INT NOT NULL DEFAULT 30 CHECK (coupon_days BETWEEN 1 AND 3650),
    max_grants         INT NOT NULL DEFAULT 0,                             -- budget: 0 = unlimited
    grants             INT NOT NULL DEFAULT 0,
    channels           TEXT[] NOT NULL DEFAULT '{web,social,pos}',
    starts_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    ends_at            TIMESTAMPTZ,
    active             BOOLEAN NOT NULL DEFAULT true,
    created_by         UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((trigger = 'code') = (code IS NOT NULL)),
    CHECK (NOT (shop_id IS NULL AND reward_type = 'points')),
    CHECK (NOT (shop_id IS NULL AND trigger = 'first_order')),
    CHECK (NOT (shop_id IS NOT NULL AND trigger = 'signup'))
);
CREATE UNIQUE INDEX promo_campaigns_code_idx ON promo_campaigns (COALESCE(shop_id, '00000000-0000-0000-0000-000000000000'::uuid), code) WHERE code IS NOT NULL;
CREATE INDEX promo_campaigns_shop_idx ON promo_campaigns (shop_id, created_at DESC);

-- A reward held by one customer (user account and/or phone). Points rewards are credited at once
-- and never become coupons.
CREATE TABLE promo_coupons (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    campaign_id        UUID NOT NULL REFERENCES promo_campaigns(id) ON DELETE CASCADE,
    shop_id            UUID REFERENCES shops(id) ON DELETE CASCADE,       -- NULL = usable at any shop
    user_id            UUID REFERENCES users(id) ON DELETE CASCADE,
    phone              TEXT,                                               -- E.164
    code               TEXT NOT NULL UNIQUE,
    reward_type        TEXT NOT NULL CHECK (reward_type IN ('amount','percent','free_shipping')),
    reward_value       BIGINT NOT NULL,
    max_discount_cents BIGINT NOT NULL DEFAULT 0,
    min_spend_cents    BIGINT NOT NULL DEFAULT 0,
    currency           TEXT NOT NULL,
    channels           TEXT[] NOT NULL,
    status             TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','used','void')),
    expires_at         TIMESTAMPTZ NOT NULL,
    used_at            TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (user_id IS NOT NULL OR phone IS NOT NULL)
);
CREATE UNIQUE INDEX promo_coupons_user_once ON promo_coupons (campaign_id, user_id) WHERE user_id IS NOT NULL;
CREATE UNIQUE INDEX promo_coupons_phone_once ON promo_coupons (campaign_id, phone) WHERE phone IS NOT NULL AND user_id IS NULL;
CREATE INDEX promo_coupons_user_idx ON promo_coupons (user_id, status);
CREATE INDEX promo_coupons_phone_idx ON promo_coupons (phone, status);

-- Per-shop loyalty programme.
CREATE TABLE loyalty_settings (
    shop_id              UUID PRIMARY KEY REFERENCES shops(id) ON DELETE CASCADE,
    enabled              BOOLEAN NOT NULL DEFAULT false,
    earn_per_cents       BIGINT NOT NULL DEFAULT 1000000 CHECK (earn_per_cents > 0),  -- spend this much → 1 point
    point_value_cents    BIGINT NOT NULL DEFAULT 10000 CHECK (point_value_cents > 0), -- 1 point = this much off
    min_redeem_points    BIGINT NOT NULL DEFAULT 10 CHECK (min_redeem_points >= 1),
    max_redeem_bps       INT NOT NULL DEFAULT 5000 CHECK (max_redeem_bps BETWEEN 1 AND 10000), -- max share of a bill paid with points
    expiry_days          INT NOT NULL DEFAULT 0 CHECK (expiry_days >= 0),            -- 0 = points don't expire
    channels             TEXT[] NOT NULL DEFAULT '{web,social,pos}',
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A shop's member = a phone number (the one thing web, chat and counter customers all have).
CREATE TABLE loyalty_members (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id          UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    phone            TEXT NOT NULL,                                        -- E.164
    name             TEXT NOT NULL DEFAULT '',
    balance          BIGINT NOT NULL DEFAULT 0 CHECK (balance >= 0),
    lifetime_points  BIGINT NOT NULL DEFAULT 0,
    orders           INT NOT NULL DEFAULT 0,
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (shop_id, phone)
);

-- Append-only points ledger.
CREATE TABLE loyalty_ledger (
    id            BIGSERIAL PRIMARY KEY,
    member_id     UUID NOT NULL REFERENCES loyalty_members(id) ON DELETE CASCADE,
    delta         BIGINT NOT NULL,
    balance_after BIGINT NOT NULL,
    reason        TEXT NOT NULL CHECK (reason IN ('earn','redeem','bonus','adjust','reverse','expire')),
    ref_type      TEXT,                                                    -- order | social | pos
    ref_id        UUID,
    note          TEXT NOT NULL DEFAULT '',
    created_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX loyalty_ledger_member_idx ON loyalty_ledger (member_id, id DESC);
CREATE INDEX loyalty_ledger_ref_idx ON loyalty_ledger (ref_type, ref_id);
CREATE UNIQUE INDEX loyalty_ledger_earn_once ON loyalty_ledger (ref_type, ref_id) WHERE reason = 'earn';
CREATE OR REPLACE FUNCTION loyalty_ledger_append_only() RETURNS trigger AS $$
BEGIN RAISE EXCEPTION 'loyalty_ledger is append-only'; END $$ LANGUAGE plpgsql;
CREATE TRIGGER loyalty_ledger_no_edit BEFORE UPDATE OR DELETE ON loyalty_ledger
    FOR EACH ROW WHEN (pg_trigger_depth() = 0) EXECUTE FUNCTION loyalty_ledger_append_only();

-- What a sale used: coupon and/or points. `reversed_at` is set when the sale is cancelled/voided.
CREATE TABLE promo_redemptions (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ref_type                TEXT NOT NULL CHECK (ref_type IN ('order','social','pos')),
    ref_id                  UUID NOT NULL,
    shop_id                 UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    coupon_id               UUID REFERENCES promo_coupons(id) ON DELETE SET NULL,
    member_id               UUID REFERENCES loyalty_members(id) ON DELETE SET NULL,
    points                  BIGINT NOT NULL DEFAULT 0,
    discount_cents          BIGINT NOT NULL DEFAULT 0,                    -- total taken off
    coupon_cents            BIGINT NOT NULL DEFAULT 0,                    -- of which by the coupon
    platform_cents          BIGINT NOT NULL DEFAULT 0,                    -- of which the platform pays
    currency                TEXT NOT NULL,
    reversed_at             TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (ref_type, ref_id)
);
CREATE INDEX promo_redemptions_shop_idx ON promo_redemptions (shop_id, created_at DESC);
