-- Module: cod_risk — reports of customers who refuse cash-on-delivery parcels, admin risk levels,
-- and a tamper-evident hash-chain ledger (optionally anchored to an external blockchain).
--
-- Privacy: customers are identified by customer_key = HMAC-SHA256(COD_RISK_PEPPER, E.164 phone).
-- Raw phone numbers and names stay in the report rows (visible to the reporting shop and admins
-- only); the ledger (cod_risk_blocks) holds keys and hashes only, never personal data.
-- The module can be switched off with COD_RISK_ENABLED=false; these tables are then just unused.

CREATE TABLE cod_risk_customers (
    customer_key     TEXT PRIMARY KEY CHECK (customer_key ~ '^[0-9a-f]{64}$'),
    phone_hint       TEXT NOT NULL DEFAULT '',            -- last 4 digits, for humans
    level            TEXT NOT NULL DEFAULT 'none' CHECK (level IN ('none','low','medium','high','blocked')),
    level_note       TEXT NOT NULL DEFAULT '',
    level_set_by     UUID REFERENCES users(id) ON DELETE SET NULL,
    level_set_at     TIMESTAMPTZ,
    level_expires_at TIMESTAMPTZ,                         -- NULL = until changed
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX cod_risk_customers_level_idx ON cod_risk_customers (level) WHERE level <> 'none';

-- Anchors: batches of blocks whose Merkle root was published to an external chain.
CREATE TABLE cod_risk_anchors (
    id           BIGSERIAL PRIMARY KEY,
    from_height  BIGINT NOT NULL,
    to_height    BIGINT NOT NULL,
    merkle_root  TEXT NOT NULL,
    driver       TEXT NOT NULL,
    tx_ref       TEXT NOT NULL DEFAULT '',                -- transaction hash / id returned by the chain gateway
    status       TEXT NOT NULL CHECK (status IN ('anchored','failed')),
    error        TEXT NOT NULL DEFAULT '',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Append-only hash chain. hash = sha256(height|prev_hash|ts_micros|event|payload).
CREATE TABLE cod_risk_blocks (
    height      BIGINT PRIMARY KEY CHECK (height > 0),
    prev_hash   TEXT NOT NULL,
    hash        TEXT NOT NULL UNIQUE,
    event       TEXT NOT NULL,
    payload     TEXT NOT NULL,                            -- canonical JSON (sorted keys), no personal data
    ts_micros   BIGINT NOT NULL,
    anchor_id   BIGINT REFERENCES cod_risk_anchors(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX cod_risk_blocks_unanchored_idx ON cod_risk_blocks (height) WHERE anchor_id IS NULL;
CREATE INDEX cod_risk_blocks_customer_idx ON cod_risk_blocks ((payload::jsonb->>'customer_key'));

-- The ledger is append-only: rows can't be edited or deleted (only anchor_id may be set once).
CREATE FUNCTION cod_risk_blocks_guard() RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'cod_risk_blocks is append-only';
    END IF;
    IF NEW.height <> OLD.height OR NEW.prev_hash <> OLD.prev_hash OR NEW.hash <> OLD.hash OR NEW.event <> OLD.event
       OR NEW.payload <> OLD.payload OR NEW.ts_micros <> OLD.ts_micros
       OR (OLD.anchor_id IS NOT NULL AND NEW.anchor_id IS DISTINCT FROM OLD.anchor_id) THEN
        RAISE EXCEPTION 'cod_risk_blocks is append-only';
    END IF;
    RETURN NEW;
END $$ LANGUAGE plpgsql;
CREATE TRIGGER cod_risk_blocks_append_only BEFORE UPDATE OR DELETE ON cod_risk_blocks
    FOR EACH ROW EXECUTE FUNCTION cod_risk_blocks_guard();

-- A seller's report about one refused COD parcel (one report per order).
CREATE TABLE cod_risk_reports (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_kind     TEXT NOT NULL CHECK (order_kind IN ('order','social')),
    order_id       UUID NOT NULL,
    order_number   TEXT NOT NULL DEFAULT '',
    shop_id        UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    reporter_id    UUID REFERENCES users(id) ON DELETE SET NULL,
    customer_key   TEXT NOT NULL REFERENCES cod_risk_customers(customer_key),
    buyer_id       UUID REFERENCES users(id) ON DELETE SET NULL,  -- marketplace buyer account, if any
    customer_name  TEXT NOT NULL DEFAULT '',
    phone          TEXT NOT NULL,                                 -- E.164, off-chain
    reason         TEXT NOT NULL CHECK (reason IN ('refused','unreachable','fake_address','no_show','changed_mind','other')),
    note           TEXT NOT NULL DEFAULT '',
    amount_cents   BIGINT NOT NULL DEFAULT 0,
    currency       TEXT NOT NULL DEFAULT 'LAK',
    carrier_code   TEXT,
    tracking_no    TEXT NOT NULL DEFAULT '',
    status         TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','confirmed','dismissed','withdrawn')),
    decision_note  TEXT NOT NULL DEFAULT '',
    decided_by     UUID REFERENCES users(id) ON DELETE SET NULL,
    decided_at     TIMESTAMPTZ,
    block_height   BIGINT REFERENCES cod_risk_blocks(height),     -- block that recorded the filing
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (order_kind, order_id)
);
CREATE INDEX cod_risk_reports_status_idx ON cod_risk_reports (status, created_at DESC);
CREATE INDEX cod_risk_reports_customer_idx ON cod_risk_reports (customer_key);
CREATE INDEX cod_risk_reports_shop_idx ON cod_risk_reports (shop_id, created_at DESC);
