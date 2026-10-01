-- Restaurant core: menu, tables with QR codes, kitchen orders (dine-in, takeaway, delivery).
CREATE SCHEMA IF NOT EXISTS restaurant;

CREATE TABLE restaurant.settings (
    shop_id            UUID PRIMARY KEY REFERENCES shops(id) ON DELETE CASCADE,
    accepting_orders   BOOLEAN NOT NULL DEFAULT true,
    dine_in            BOOLEAN NOT NULL DEFAULT true,
    takeaway           BOOLEAN NOT NULL DEFAULT true,
    delivery           BOOLEAN NOT NULL DEFAULT false,
    service_charge_bps INT NOT NULL DEFAULT 0 CHECK (service_charge_bps BETWEEN 0 AND 3000),
    prep_minutes       INT NOT NULL DEFAULT 15 CHECK (prep_minutes BETWEEN 0 AND 240),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE restaurant.sections (
    id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id  UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    name     TEXT NOT NULL,
    name_lo  TEXT NOT NULL DEFAULT '',
    position INT NOT NULL DEFAULT 0
);
CREATE INDEX sections_shop_idx ON restaurant.sections (shop_id, position);

CREATE TABLE restaurant.items (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id     UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    section_id  UUID REFERENCES restaurant.sections(id) ON DELETE SET NULL,
    name        TEXT NOT NULL,
    name_lo     TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    price_cents BIGINT NOT NULL CHECK (price_cents >= 0),
    image_url   TEXT NOT NULL DEFAULT '',
    spicy       SMALLINT NOT NULL DEFAULT 0 CHECK (spicy BETWEEN 0 AND 3),
    available   BOOLEAN NOT NULL DEFAULT true,
    position    INT NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX items_shop_idx ON restaurant.items (shop_id, section_id, position);

CREATE TABLE restaurant.tables (
    id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    label   TEXT NOT NULL,
    seats   INT NOT NULL DEFAULT 4 CHECK (seats BETWEEN 1 AND 100),
    -- printed in the table's QR code: /r/t/<token>
    token   TEXT NOT NULL UNIQUE,
    active  BOOLEAN NOT NULL DEFAULT true,
    UNIQUE (shop_id, label)
);

CREATE TABLE restaurant.orders (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id        UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    -- ticket number, restarts every day (shop-local)
    number         INT NOT NULL,
    day            DATE NOT NULL,
    mode           TEXT NOT NULL CHECK (mode IN ('dine_in','takeaway','delivery')),
    table_id       UUID REFERENCES restaurant.tables(id) ON DELETE SET NULL,
    table_label    TEXT NOT NULL DEFAULT '',
    user_id        UUID REFERENCES users(id) ON DELETE SET NULL,
    customer_name  TEXT NOT NULL DEFAULT '',
    customer_phone TEXT NOT NULL DEFAULT '',
    address        TEXT NOT NULL DEFAULT '',
    note           TEXT NOT NULL DEFAULT '',
    status         TEXT NOT NULL DEFAULT 'new'
        CHECK (status IN ('new','accepted','preparing','ready','served','completed','cancelled')),
    subtotal_cents BIGINT NOT NULL,
    service_cents  BIGINT NOT NULL DEFAULT 0,
    total_cents    BIGINT NOT NULL,
    currency       TEXT NOT NULL,
    paid           BOOLEAN NOT NULL DEFAULT false,
    payment_method TEXT NOT NULL DEFAULT '' CHECK (payment_method IN ('','cash','transfer','card')),
    -- customer's link to follow the order: /r/o/<track_token>
    track_token    TEXT NOT NULL UNIQUE,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (shop_id, day, number)
);
CREATE INDEX rorders_board_idx ON restaurant.orders (shop_id, status, created_at);

CREATE TABLE restaurant.order_items (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id         UUID NOT NULL REFERENCES restaurant.orders(id) ON DELETE CASCADE,
    item_id          UUID REFERENCES restaurant.items(id) ON DELETE SET NULL,
    name             TEXT NOT NULL,
    unit_price_cents BIGINT NOT NULL,
    qty              INT NOT NULL CHECK (qty BETWEEN 1 AND 99),
    note             TEXT NOT NULL DEFAULT ''
);
CREATE INDEX ritems_order_idx ON restaurant.order_items (order_id);
