-- zaokaiy core schema. Money is stored as BIGINT minor units (cents/satang).
-- Commission rates are basis points (bps): 1000 = 10.00%.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    role          TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('user','admin')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE shops (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    slug        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    logo_url    TEXT,
    currency    TEXT NOT NULL DEFAULT 'THB',
    -- 'creator' shops are content creators who mostly resell others' products
    kind        TEXT NOT NULL DEFAULT 'seller' CHECK (kind IN ('seller','creator')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX shops_owner_idx ON shops(owner_id);

CREATE TABLE products (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id             UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    sku                 TEXT NOT NULL,
    name                TEXT NOT NULL,
    description         TEXT NOT NULL DEFAULT '',
    price_cents         BIGINT NOT NULL CHECK (price_cents >= 0),
    images              JSONB NOT NULL DEFAULT '[]',
    category            TEXT NOT NULL DEFAULT 'general',
    status              TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','active','archived')),
    stock               INT  NOT NULL DEFAULT 0 CHECK (stock >= 0),
    low_stock_threshold INT  NOT NULL DEFAULT 5,
    -- Reseller ("sell staff") program
    allow_resell        BOOLEAN NOT NULL DEFAULT false,
    commission_bps      INT NOT NULL DEFAULT 1000 CHECK (commission_bps BETWEEN 0 AND 9000),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (shop_id, sku)
);
CREATE INDEX products_status_idx ON products(status);
CREATE INDEX products_resell_idx ON products(allow_resell) WHERE allow_resell;

CREATE TABLE inventory_movements (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id   UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    delta        INT  NOT NULL,
    stock_after  INT  NOT NULL,
    reason       TEXT NOT NULL CHECK (reason IN ('restock','sale','adjust','return','cancel')),
    ref_order_id UUID,
    note         TEXT NOT NULL DEFAULT '',
    created_by   UUID REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX inv_mov_product_idx ON inventory_movements(product_id, created_at DESC);

-- A reseller shop asks a supplier shop for permission to sell its products.
-- The supplier approves and may override the per-product commission.
CREATE TABLE partnerships (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    supplier_shop_id   UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    reseller_shop_id   UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    status             TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','revoked')),
    commission_bps     INT CHECK (commission_bps BETWEEN 0 AND 9000), -- NULL = use product default
    message            TEXT NOT NULL DEFAULT '',
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at         TIMESTAMPTZ,
    UNIQUE (supplier_shop_id, reseller_shop_id),
    CHECK (supplier_shop_id <> reseller_shop_id)
);

-- Products a reseller shows in its own storefront.
CREATE TABLE listings (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    reseller_shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    product_id       UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    active           BOOLEAN NOT NULL DEFAULT true,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (reseller_shop_id, product_id)
);

CREATE TABLE orders (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    buyer_id         UUID NOT NULL REFERENCES users(id),
    status           TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','paid','shipped','completed','cancelled')),
    total_cents      BIGINT NOT NULL,
    currency         TEXT NOT NULL DEFAULT 'THB',
    shipping_address JSONB NOT NULL DEFAULT '{}',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX orders_buyer_idx ON orders(buyer_id, created_at DESC);

CREATE TABLE order_items (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id          UUID NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    product_id        UUID NOT NULL REFERENCES products(id),
    product_name      TEXT NOT NULL,
    supplier_shop_id  UUID NOT NULL REFERENCES shops(id),
    seller_shop_id    UUID REFERENCES shops(id),         -- reseller that made the sale (NULL = direct)
    qty               INT NOT NULL CHECK (qty > 0),
    unit_price_cents  BIGINT NOT NULL,
    commission_bps    INT NOT NULL DEFAULT 0,
    commission_cents  BIGINT NOT NULL DEFAULT 0
);
CREATE INDEX order_items_supplier_idx ON order_items(supplier_shop_id);
CREATE INDEX order_items_seller_idx ON order_items(seller_shop_id);

CREATE TABLE commissions (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_item_id       UUID NOT NULL REFERENCES order_items(id) ON DELETE CASCADE,
    beneficiary_shop_id UUID NOT NULL REFERENCES shops(id),
    supplier_shop_id    UUID NOT NULL REFERENCES shops(id),
    amount_cents        BIGINT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','paid','void')),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX commissions_beneficiary_idx ON commissions(beneficiary_shop_id, status);

CREATE TABLE ad_campaigns (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id       UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    product_id    UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    headline      TEXT NOT NULL,
    body          TEXT NOT NULL DEFAULT '',
    image_url     TEXT,
    budget_cents  BIGINT NOT NULL CHECK (budget_cents >= 0),
    cpc_cents     BIGINT NOT NULL DEFAULT 100 CHECK (cpc_cents > 0),
    spent_cents   BIGINT NOT NULL DEFAULT 0,
    impressions   BIGINT NOT NULL DEFAULT 0,
    clicks        BIGINT NOT NULL DEFAULT 0,
    status        TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','active','paused','ended')),
    starts_at     TIMESTAMPTZ,
    ends_at       TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX ads_active_idx ON ad_campaigns(status) WHERE status = 'active';

-- Creator content: posts, captions, video scripts that link to a product.
CREATE TABLE contents (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id      UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    product_id   UUID REFERENCES products(id) ON DELETE SET NULL,
    kind         TEXT NOT NULL CHECK (kind IN ('post','caption','video_script','description','ad_copy')),
    title        TEXT NOT NULL,
    body         TEXT NOT NULL,
    media_url    TEXT,
    ai_generated BOOLEAN NOT NULL DEFAULT false,
    status       TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','published')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX contents_feed_idx ON contents(status, created_at DESC);
