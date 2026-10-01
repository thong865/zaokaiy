-- Module pos_sync: desktop POS terminals (Windows / macOS app) that keep selling offline and sync later.
--
-- * A terminal is paired once from the web (device-authorisation flow: the app shows a code, a
--   signed-in owner/cashier approves it at /pos/pair) and then holds a long-lived device token.
-- * Each terminal numbers its own receipts gap-free (`<receipt_prefix><code>-000001`), so a receipt
--   printed offline already carries its final number.
-- * Catalogue changes are pulled incrementally with transaction-id cursors (xid8), which never miss
--   a row committed late by a long transaction; deletions come from a tombstone table.
-- * Sales are pushed with the id the terminal generated, so re-sending after a timeout is harmless.

CREATE TABLE pos_devices (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id       UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,   -- the cashier the terminal acts as
    code          TEXT NOT NULL,                                          -- T01, T02 … (part of receipt numbers)
    name          TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    platform      TEXT NOT NULL DEFAULT '',
    app_version   TEXT NOT NULL DEFAULT '',
    install_id    TEXT NOT NULL,
    token_hash    TEXT NOT NULL UNIQUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at  TIMESTAMPTZ,
    last_push_at  TIMESTAMPTZ,
    revoked_at    TIMESTAMPTZ,
    UNIQUE (shop_id, code)
);
CREATE UNIQUE INDEX pos_devices_install_idx ON pos_devices (shop_id, install_id) WHERE revoked_at IS NULL;

CREATE TABLE pos_pairings (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code         TEXT NOT NULL UNIQUE,                 -- shown on the terminal, typed/opened on the web
    poll_hash    TEXT NOT NULL,                        -- sha256 of the terminal's poll secret
    name         TEXT NOT NULL,
    platform     TEXT NOT NULL DEFAULT '',
    app_version  TEXT NOT NULL DEFAULT '',
    install_id   TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL,
    approved_by  UUID REFERENCES users(id) ON DELETE CASCADE,
    shop_id      UUID REFERENCES shops(id) ON DELETE CASCADE,
    approved_at  TIMESTAMPTZ,
    consumed_at  TIMESTAMPTZ
);

ALTER TABLE pos_sales
    ADD COLUMN origin          TEXT NOT NULL DEFAULT 'web' CHECK (origin IN ('web','device')),
    ADD COLUMN device_id       UUID REFERENCES pos_devices(id) ON DELETE SET NULL,
    ADD COLUMN device_seq      BIGINT,
    ADD COLUMN synced_at       TIMESTAMPTZ,
    -- Lines sold offline beyond the stock the server had: [{product_id, name, qty}]
    ADD COLUMN stock_shortfall JSONB NOT NULL DEFAULT '[]';
CREATE UNIQUE INDEX pos_sales_device_seq_idx ON pos_sales (device_id, device_seq) WHERE device_id IS NOT NULL;

-- Change tracking for the terminals' catalogue copy.
ALTER TABLE products ADD COLUMN sync_txid xid8 NOT NULL DEFAULT pg_current_xact_id();
CREATE INDEX products_shop_sync_idx ON products (shop_id, sync_txid, id);

CREATE FUNCTION pos_sync_touch() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    NEW.sync_txid := pg_current_xact_id();
    RETURN NEW;
END $$;
CREATE TRIGGER products_pos_sync_touch BEFORE INSERT OR UPDATE ON products
    FOR EACH ROW EXECUTE FUNCTION pos_sync_touch();

CREATE TABLE pos_sync_tombstones (
    product_id UUID PRIMARY KEY,
    shop_id    UUID NOT NULL,
    sync_txid  xid8 NOT NULL DEFAULT pg_current_xact_id(),
    deleted_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX pos_sync_tombstones_shop_idx ON pos_sync_tombstones (shop_id, sync_txid);

CREATE FUNCTION pos_sync_tombstone() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO pos_sync_tombstones (product_id, shop_id) VALUES (OLD.id, OLD.shop_id)
    ON CONFLICT (product_id) DO UPDATE SET sync_txid = pg_current_xact_id(), deleted_at = now();
    RETURN OLD;
END $$;
CREATE TRIGGER products_pos_sync_tombstone AFTER DELETE ON products
    FOR EACH ROW EXECUTE FUNCTION pos_sync_tombstone();
-- Shop settings and shop categories are small and sent in full on every pull (no cursor needed).
