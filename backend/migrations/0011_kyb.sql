-- Corporate KYC (know your business) for business shops.
--   shops.entity_type = 'business' → the shop must pass verification before it can publish
--   products (KYB_REQUIRED_TO_PUBLISH). Individuals are unaffected.
--   Identity and bank numbers are stored encrypted (AES-256-GCM, KYC_ENCRYPTION_KEY) with the
--   last 4 characters in clear for display; documents are encrypted files in private storage.

ALTER TABLE shops
    ADD COLUMN entity_type     TEXT NOT NULL DEFAULT 'individual' CHECK (entity_type IN ('individual','business')),
    ADD COLUMN kyb_status      TEXT NOT NULL DEFAULT 'none'
        CHECK (kyb_status IN ('none','draft','submitted','changes_requested','approved','rejected','revoked')),
    ADD COLUMN kyb_verified_at TIMESTAMPTZ;

CREATE TABLE kyb_profiles (
    shop_id             UUID PRIMARY KEY REFERENCES shops(id) ON DELETE CASCADE,
    company_type        TEXT NOT NULL DEFAULT '',
    legal_name          TEXT NOT NULL DEFAULT '',
    legal_name_local    TEXT NOT NULL DEFAULT '',
    registration_no     TEXT NOT NULL DEFAULT '',
    registration_date   DATE,
    tax_id              TEXT NOT NULL DEFAULT '',
    country             TEXT NOT NULL DEFAULT 'LA',
    province            TEXT NOT NULL DEFAULT '',
    registered_address  TEXT NOT NULL DEFAULT '',
    business_activity   TEXT NOT NULL DEFAULT '',
    website             TEXT NOT NULL DEFAULT '',
    contact_email       TEXT NOT NULL DEFAULT '',
    contact_phone       TEXT NOT NULL DEFAULT '',
    bank_name           TEXT NOT NULL DEFAULT '',
    bank_account_name   TEXT NOT NULL DEFAULT '',
    bank_account_enc    TEXT NOT NULL DEFAULT '',
    bank_account_last4  TEXT NOT NULL DEFAULT '',
    risk_flags          TEXT[] NOT NULL DEFAULT '{}',
    submitted_at        TIMESTAMPTZ,
    reviewed_at         TIMESTAMPTZ,
    reviewed_by         UUID REFERENCES users(id) ON DELETE SET NULL,
    review_note         TEXT NOT NULL DEFAULT '',
    review_due_at       TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX kyb_profiles_regno_idx ON kyb_profiles (country, lower(registration_no)) WHERE registration_no <> '';

-- Representative, directors and ultimate beneficial owners (one person can hold several roles).
CREATE TABLE kyb_persons (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id        UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    position_no    INT  NOT NULL DEFAULT 0,
    roles          TEXT[] NOT NULL DEFAULT '{}',
    full_name      TEXT NOT NULL,
    title          TEXT NOT NULL DEFAULT '',
    nationality    TEXT NOT NULL DEFAULT 'LA',
    date_of_birth  DATE,
    id_type        TEXT NOT NULL DEFAULT 'national_id'
                   CHECK (id_type IN ('national_id','passport','family_book','other')),
    id_number_enc  TEXT NOT NULL DEFAULT '',
    id_last4       TEXT NOT NULL DEFAULT '',
    ownership_bps  INT  NOT NULL DEFAULT 0 CHECK (ownership_bps BETWEEN 0 AND 10000),
    is_pep         BOOLEAN NOT NULL DEFAULT false,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX kyb_persons_shop_idx ON kyb_persons (shop_id, position_no);

CREATE TABLE kyb_documents (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id       UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL CHECK (kind IN ('registration_certificate','tax_certificate','business_license',
                    'representative_id','authorization_letter','shareholder_register','articles','proof_of_address',
                    'bank_proof','other')),
    storage_key   TEXT NOT NULL,
    content_type  TEXT NOT NULL,
    size_bytes    BIGINT NOT NULL,
    sha256        TEXT NOT NULL,
    original_name TEXT NOT NULL DEFAULT '',
    expires_on    DATE,
    status        TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','accepted','rejected')),
    review_note   TEXT NOT NULL DEFAULT '',
    uploaded_by   UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX kyb_documents_shop_idx ON kyb_documents (shop_id, created_at);

-- Audit trail: saves, uploads, submissions, admin views and decisions.
CREATE TABLE kyb_events (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id    UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    actor_id   UUID REFERENCES users(id) ON DELETE SET NULL,
    action     TEXT NOT NULL,
    note       TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX kyb_events_shop_idx ON kyb_events (shop_id, created_at DESC);
CREATE INDEX shops_kyb_status_idx ON shops (kyb_status) WHERE kyb_status IN ('submitted','changes_requested');
