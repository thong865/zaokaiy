-- Social commerce: comment-to-order from Facebook, TikTok, WhatsApp (and any webhook source).

-- Short code customers type in comments, e.g. "CF A01 x2".
ALTER TABLE products ADD COLUMN social_code TEXT;
CREATE UNIQUE INDEX products_shop_social_code_idx ON products(shop_id, upper(social_code))
    WHERE social_code IS NOT NULL AND social_code <> '';

-- Stock holds for social claims.
ALTER TABLE inventory_movements DROP CONSTRAINT IF EXISTS inventory_movements_reason_check;
ALTER TABLE inventory_movements ADD CONSTRAINT inventory_movements_reason_check
    CHECK (reason IN ('restock','sale','adjust','return','cancel','reserve','release'));

-- Per-shop social selling settings.
ALTER TABLE shops
    ADD COLUMN social_triggers       TEXT[] NOT NULL DEFAULT ARRAY['cf','f','เอา','รับ','สั่ง','order','buy'],
    ADD COLUMN social_require_trigger BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN social_hold_hours     INT NOT NULL DEFAULT 24 CHECK (social_hold_hours BETWEEN 1 AND 336),
    ADD COLUMN social_max_qty        INT NOT NULL DEFAULT 10 CHECK (social_max_qty BETWEEN 1 AND 999),
    ADD COLUMN social_shipping_cents BIGINT NOT NULL DEFAULT 0 CHECK (social_shipping_cents >= 0),
    ADD COLUMN social_reply_template TEXT NOT NULL DEFAULT
        'รับออเดอร์แล้วค่ะ {name} ✅ {items} รวม {total} กรุณายืนยันและชำระเงินที่ {link} ภายใน {hours} ชม.',
    ADD COLUMN social_soldout_template TEXT NOT NULL DEFAULT 'ขออภัยค่ะ {name} {items} หมดแล้ว 🙏',
    ADD COLUMN payment_instructions  TEXT NOT NULL DEFAULT '';

-- Connected accounts. Tokens are write-only through the API (never returned).
CREATE TABLE social_channels (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id      UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    provider     TEXT NOT NULL CHECK (provider IN ('facebook','tiktok','whatsapp','webhook')),
    name         TEXT NOT NULL,
    external_id  TEXT NOT NULL DEFAULT '',   -- FB page id / WhatsApp phone_number_id / TikTok open_id
    access_token TEXT NOT NULL DEFAULT '',
    secret       TEXT NOT NULL,               -- HMAC secret for the generic ingest webhook
    active       BOOLEAN NOT NULL DEFAULT true,
    auto_reply   BOOLEAN NOT NULL DEFAULT true,
    last_event_at TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX social_channels_ext_idx ON social_channels(provider, external_id) WHERE external_id <> '';

-- Live selling sessions (a Facebook/TikTok live, a post campaign…).
CREATE TABLE social_sessions (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id    UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'live' CHECK (status IN ('live','ended')),
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at   TIMESTAMPTZ
);
CREATE UNIQUE INDEX social_sessions_one_live_idx ON social_sessions(shop_id) WHERE status = 'live';

CREATE TABLE social_customers (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id          UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    provider         TEXT NOT NULL,
    external_user_id TEXT NOT NULL,
    name             TEXT NOT NULL DEFAULT '',
    phone            TEXT NOT NULL DEFAULT '',
    address          TEXT NOT NULL DEFAULT '',
    blocked          BOOLEAN NOT NULL DEFAULT false,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (shop_id, provider, external_user_id)
);

CREATE TABLE social_orders (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id        UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    customer_id    UUID NOT NULL REFERENCES social_customers(id) ON DELETE CASCADE,
    channel_id     UUID REFERENCES social_channels(id) ON DELETE SET NULL,
    session_id     UUID REFERENCES social_sessions(id) ON DELETE SET NULL,
    number         TEXT NOT NULL,
    -- open: collecting claims · confirmed: customer submitted checkout · paid · shipped · completed
    status         TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open','confirmed','paid','shipped','completed','cancelled','expired')),
    token          TEXT NOT NULL UNIQUE,          -- secret for the customer checkout link
    subtotal_cents BIGINT NOT NULL DEFAULT 0,
    shipping_cents BIGINT NOT NULL DEFAULT 0,
    total_cents    BIGINT NOT NULL DEFAULT 0,
    currency       TEXT NOT NULL DEFAULT 'THB',
    ship_name      TEXT NOT NULL DEFAULT '',
    ship_phone     TEXT NOT NULL DEFAULT '',
    ship_address   TEXT NOT NULL DEFAULT '',
    customer_note  TEXT NOT NULL DEFAULT '',
    payment_method TEXT NOT NULL DEFAULT '',
    payment_ref    TEXT NOT NULL DEFAULT '',
    tracking_no    TEXT NOT NULL DEFAULT '',
    expires_at     TIMESTAMPTZ NOT NULL,
    confirmed_at   TIMESTAMPTZ,
    paid_at        TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (shop_id, number)
);
CREATE INDEX social_orders_shop_idx ON social_orders(shop_id, created_at DESC);
CREATE UNIQUE INDEX social_orders_one_open_idx ON social_orders(customer_id) WHERE status = 'open';
CREATE INDEX social_orders_expiry_idx ON social_orders(expires_at) WHERE status IN ('open','confirmed');

CREATE TABLE social_order_items (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id         UUID NOT NULL REFERENCES social_orders(id) ON DELETE CASCADE,
    product_id       UUID REFERENCES products(id) ON DELETE SET NULL,
    code             TEXT NOT NULL,
    name             TEXT NOT NULL,
    qty              INT NOT NULL CHECK (qty > 0),
    unit_price_cents BIGINT NOT NULL,
    UNIQUE (order_id, product_id)
);

-- Every inbound comment/message (deduplicated), with what we did about it.
CREATE TABLE social_comments (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id          UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    channel_id       UUID REFERENCES social_channels(id) ON DELETE SET NULL,
    session_id       UUID REFERENCES social_sessions(id) ON DELETE SET NULL,
    provider         TEXT NOT NULL,
    kind             TEXT NOT NULL DEFAULT 'comment' CHECK (kind IN ('comment','message','live')),
    external_id      TEXT NOT NULL,
    external_user_id TEXT NOT NULL,
    user_name        TEXT NOT NULL DEFAULT '',
    post_id          TEXT NOT NULL DEFAULT '',
    message          TEXT NOT NULL,
    parsed           JSONB NOT NULL DEFAULT '[]',
    result           TEXT NOT NULL DEFAULT 'ignored'
        CHECK (result IN ('ignored','claimed','partial','sold_out','blocked','error')),
    order_id         UUID REFERENCES social_orders(id) ON DELETE SET NULL,
    reply_text       TEXT NOT NULL DEFAULT '',
    reply_status     TEXT NOT NULL DEFAULT 'none' CHECK (reply_status IN ('none','sent','simulated','failed','unsupported')),
    reply_error      TEXT NOT NULL DEFAULT '',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (shop_id, provider, external_id)
);
CREATE INDEX social_comments_feed_idx ON social_comments(shop_id, created_at DESC);

ALTER TABLE inventory_movements ADD COLUMN ref_social_order_id UUID REFERENCES social_orders(id) ON DELETE SET NULL;
