-- Point of sale (counter selling), receipts and tax invoices.

-- Barcodes for scanning at the counter (EAN/UPC or any code the seller prints).
ALTER TABLE products ADD COLUMN barcode TEXT;
CREATE UNIQUE INDEX products_shop_barcode_idx ON products(shop_id, barcode) WHERE barcode IS NOT NULL AND barcode <> '';

-- Business & tax details printed on receipts/invoices.
ALTER TABLE shops
    ADD COLUMN legal_name          TEXT NOT NULL DEFAULT '',
    ADD COLUMN tax_id              TEXT NOT NULL DEFAULT '',
    ADD COLUMN branch              TEXT NOT NULL DEFAULT '',      -- e.g. "Head office" / "สำนักงานใหญ่"
    ADD COLUMN address             TEXT NOT NULL DEFAULT '',
    ADD COLUMN phone               TEXT NOT NULL DEFAULT '',
    ADD COLUMN vat_bps             INT  NOT NULL DEFAULT 700 CHECK (vat_bps BETWEEN 0 AND 3000), -- 700 = 7%
    ADD COLUMN prices_include_vat  BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN receipt_prefix      TEXT NOT NULL DEFAULT 'R',
    ADD COLUMN invoice_prefix      TEXT NOT NULL DEFAULT 'INV',
    ADD COLUMN receipt_footer      TEXT NOT NULL DEFAULT 'Thank you!';

-- Gap-free document numbering per shop.
CREATE TABLE shop_counters (
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    kind    TEXT NOT NULL,           -- 'receipt' | 'invoice'
    next    BIGINT NOT NULL DEFAULT 1,
    PRIMARY KEY (shop_id, kind)
);

CREATE TABLE pos_sales (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id          UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    number           TEXT NOT NULL,
    cashier_id       UUID REFERENCES users(id) ON DELETE SET NULL,
    status           TEXT NOT NULL DEFAULT 'completed' CHECK (status IN ('completed','voided')),
    subtotal_cents   BIGINT NOT NULL,   -- sum of lines after line discounts
    discount_cents   BIGINT NOT NULL DEFAULT 0, -- bill-level discount
    vat_bps          INT    NOT NULL,
    prices_include_vat BOOLEAN NOT NULL,
    vat_cents        BIGINT NOT NULL,
    total_cents      BIGINT NOT NULL,   -- amount charged
    paid_cents       BIGINT NOT NULL,
    change_cents     BIGINT NOT NULL DEFAULT 0,
    payments         JSONB  NOT NULL DEFAULT '[]', -- [{method, amount_cents, ref}]
    currency         TEXT   NOT NULL DEFAULT 'THB',
    note             TEXT   NOT NULL DEFAULT '',
    -- Full tax invoice (issued on request)
    invoice_number   TEXT,
    invoice_issued_at TIMESTAMPTZ,
    customer_name    TEXT NOT NULL DEFAULT '',
    customer_tax_id  TEXT NOT NULL DEFAULT '',
    customer_branch  TEXT NOT NULL DEFAULT '',
    customer_address TEXT NOT NULL DEFAULT '',
    customer_phone   TEXT NOT NULL DEFAULT '',
    void_reason      TEXT NOT NULL DEFAULT '',
    voided_at        TIMESTAMPTZ,
    voided_by        UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (shop_id, number)
);
CREATE UNIQUE INDEX pos_sales_invoice_idx ON pos_sales(shop_id, invoice_number) WHERE invoice_number IS NOT NULL;
CREATE INDEX pos_sales_shop_time_idx ON pos_sales(shop_id, created_at DESC);

CREATE TABLE pos_sale_items (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sale_id          UUID NOT NULL REFERENCES pos_sales(id) ON DELETE CASCADE,
    product_id       UUID REFERENCES products(id) ON DELETE SET NULL, -- NULL = custom/open item
    name             TEXT NOT NULL,
    sku              TEXT NOT NULL DEFAULT '',
    qty              INT  NOT NULL CHECK (qty > 0),
    unit_price_cents BIGINT NOT NULL CHECK (unit_price_cents >= 0),
    discount_cents   BIGINT NOT NULL DEFAULT 0 CHECK (discount_cents >= 0),
    line_total_cents BIGINT NOT NULL,
    position         INT NOT NULL DEFAULT 0
);
CREATE INDEX pos_sale_items_sale_idx ON pos_sale_items(sale_id);

-- Allow POS stock movements to reference the sale.
ALTER TABLE inventory_movements ADD COLUMN ref_pos_sale_id UUID REFERENCES pos_sales(id) ON DELETE SET NULL;
