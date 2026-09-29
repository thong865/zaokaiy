-- Delivery by Lao couriers (Anousith, HAL, Mixay …) and cash on delivery (COD).
-- The couriers have no public booking API: the seller drops the parcel at the courier, types the
-- tracking number, and records COD collection / remittance here.

CREATE TABLE carriers (
    code         TEXT PRIMARY KEY CHECK (code ~ '^[a-z0-9_]{2,30}$'),
    name         TEXT NOT NULL,
    name_lo      TEXT NOT NULL DEFAULT '',
    website      TEXT NOT NULL DEFAULT '',
    -- Public tracking page. "{tracking}" is replaced by the tracking number; empty = no link.
    tracking_url TEXT NOT NULL DEFAULT '',
    phone        TEXT NOT NULL DEFAULT '',
    supports_cod BOOLEAN NOT NULL DEFAULT true,
    active       BOOLEAN NOT NULL DEFAULT true,
    sort         INT NOT NULL DEFAULT 0,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO carriers (code, name, name_lo, website, sort) VALUES
    ('anousith', 'Anousith Express', 'ອານຸສິດ ຂົນສົ່ງດ່ວນ', '', 10),
    ('hal',      'HAL Express',      'ຮຸ່ງອາລຸນ ຂົນສົ່ງດ່ວນ', 'https://hal-logistics.la', 20),
    ('mixay',    'Mixay Express',    'ມີໄຊ ຂົນສົ່ງ',          'https://www.mixayexpresslao.com', 30),
    ('self',     'Shop delivery',    'ຮ້ານສົ່ງເອງ',            '', 90);

-- Which couriers a shop uses and on what terms.
CREATE TABLE shop_shipping (
    shop_id        UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    carrier_code   TEXT NOT NULL REFERENCES carriers(code) ON UPDATE CASCADE,
    enabled        BOOLEAN NOT NULL DEFAULT true,
    -- buyer: fee added at checkout · destination: buyer pays the courier when collecting (ປາຍທາງ) · seller: free shipping
    fee_payer      TEXT NOT NULL DEFAULT 'buyer' CHECK (fee_payer IN ('buyer','destination','seller')),
    fee_cents      BIGINT NOT NULL DEFAULT 0 CHECK (fee_cents >= 0),
    home_fee_cents BIGINT CHECK (home_fee_cents >= 0),        -- NULL = no home delivery, branch pickup only
    free_over_cents BIGINT CHECK (free_over_cents > 0),       -- free shipping above this subtotal
    cod_enabled    BOOLEAN NOT NULL DEFAULT false,
    cod_fee_cents  BIGINT NOT NULL DEFAULT 0 CHECK (cod_fee_cents >= 0),
    eta            TEXT NOT NULL DEFAULT '',
    note           TEXT NOT NULL DEFAULT '',
    sort           INT NOT NULL DEFAULT 0,
    PRIMARY KEY (shop_id, carrier_code)
);

-- Delivery + COD on marketplace orders.
ALTER TABLE orders
    ADD COLUMN carrier_code       TEXT REFERENCES carriers(code) ON UPDATE CASCADE,
    ADD COLUMN delivery_type      TEXT NOT NULL DEFAULT 'branch' CHECK (delivery_type IN ('branch','home')),
    ADD COLUMN fee_payer          TEXT NOT NULL DEFAULT 'buyer' CHECK (fee_payer IN ('buyer','destination','seller')),
    ADD COLUMN shipping_fee_cents BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN payment_method     TEXT NOT NULL DEFAULT 'prepaid' CHECK (payment_method IN ('prepaid','cod')),
    ADD COLUMN cod_fee_cents      BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN cod_amount_cents   BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN cod_status         TEXT NOT NULL DEFAULT 'none' CHECK (cod_status IN ('none','pending','collected','remitted','returned')),
    ADD COLUMN cod_remit_ref      TEXT NOT NULL DEFAULT '',
    ADD COLUMN cod_collected_at   TIMESTAMPTZ,
    ADD COLUMN cod_remitted_at    TIMESTAMPTZ,
    ADD COLUMN tracking_no        TEXT NOT NULL DEFAULT '',
    ADD COLUMN shipped_at         TIMESTAMPTZ;
-- What the buyer pays in total (items + shipping when charged at checkout + COD fee).
ALTER TABLE orders ADD COLUMN grand_total_cents BIGINT GENERATED ALWAYS AS
    (total_cents + CASE WHEN fee_payer = 'buyer' THEN shipping_fee_cents ELSE 0 END + cod_fee_cents) STORED;
CREATE INDEX orders_cod_idx ON orders (cod_status) WHERE cod_status <> 'none';

-- Same for comment/chat orders (their shipping_cents already holds the fee charged at checkout).
ALTER TABLE social_orders
    ADD COLUMN carrier_code     TEXT REFERENCES carriers(code) ON UPDATE CASCADE,
    ADD COLUMN delivery_type    TEXT NOT NULL DEFAULT 'branch' CHECK (delivery_type IN ('branch','home')),
    ADD COLUMN fee_payer        TEXT NOT NULL DEFAULT 'buyer' CHECK (fee_payer IN ('buyer','destination','seller')),
    ADD COLUMN cod_fee_cents    BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN cod_amount_cents BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN cod_status       TEXT NOT NULL DEFAULT 'none' CHECK (cod_status IN ('none','pending','collected','remitted','returned')),
    ADD COLUMN cod_remit_ref    TEXT NOT NULL DEFAULT '',
    ADD COLUMN cod_collected_at TIMESTAMPTZ,
    ADD COLUMN cod_remitted_at  TIMESTAMPTZ,
    ADD COLUMN shipped_at       TIMESTAMPTZ;
CREATE INDEX social_orders_cod_idx ON social_orders (cod_status) WHERE cod_status <> 'none';

-- A COD parcel refused by the customer comes back: stock is returned with reason 'return'.
