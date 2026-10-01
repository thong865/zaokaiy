-- Insurance core: agents / brokers publish plans; customers get a quote and apply online;
-- the agent reviews the application and issues the policy.
CREATE SCHEMA IF NOT EXISTS insurance;

CREATE TABLE insurance.plans (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id                UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    kind                   TEXT NOT NULL CHECK (kind IN ('motor','health','travel','life','property','accident','other')),
    insurer                TEXT NOT NULL,
    name                   TEXT NOT NULL,
    name_lo                TEXT NOT NULL DEFAULT '',
    description            TEXT NOT NULL DEFAULT '',
    coverage               TEXT[] NOT NULL DEFAULT '{}',
    -- fixed: premium_cents per term · rate: rate_bps of the sum insured (at least min_premium_cents)
    premium_mode           TEXT NOT NULL DEFAULT 'fixed' CHECK (premium_mode IN ('fixed','rate')),
    premium_cents          BIGINT NOT NULL DEFAULT 0 CHECK (premium_cents >= 0),
    rate_bps               INT NOT NULL DEFAULT 0 CHECK (rate_bps BETWEEN 0 AND 10000),
    min_premium_cents      BIGINT NOT NULL DEFAULT 0 CHECK (min_premium_cents >= 0),
    sum_insured_min_cents  BIGINT NOT NULL DEFAULT 0 CHECK (sum_insured_min_cents >= 0),
    sum_insured_max_cents  BIGINT NOT NULL DEFAULT 0 CHECK (sum_insured_max_cents >= 0),
    term_months            INT NOT NULL DEFAULT 12 CHECK (term_months BETWEEN 1 AND 120),
    active                 BOOLEAN NOT NULL DEFAULT true,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX plans_kind_idx ON insurance.plans (kind) WHERE active;

CREATE TABLE insurance.applications (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    number            TEXT NOT NULL UNIQUE,
    plan_id           UUID NOT NULL REFERENCES insurance.plans(id),
    shop_id           UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    user_id           UUID REFERENCES users(id) ON DELETE SET NULL,
    applicant_name    TEXT NOT NULL,
    phone             TEXT NOT NULL,
    email             TEXT NOT NULL DEFAULT '',
    -- what is insured, per kind: plate / make / model for motor, destination and dates for travel …
    details           JSONB NOT NULL DEFAULT '{}',
    sum_insured_cents BIGINT NOT NULL DEFAULT 0,
    premium_cents     BIGINT NOT NULL,
    currency          TEXT NOT NULL,
    start_date        DATE NOT NULL,
    end_date          DATE NOT NULL,
    status            TEXT NOT NULL DEFAULT 'submitted'
        CHECK (status IN ('submitted','reviewing','approved','issued','rejected','cancelled')),
    policy_no         TEXT NOT NULL DEFAULT '',
    paid              BOOLEAN NOT NULL DEFAULT false,
    decision_note     TEXT NOT NULL DEFAULT '',
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX apps_shop_idx ON insurance.applications (shop_id, status, created_at DESC);
CREATE INDEX apps_user_idx ON insurance.applications (user_id, created_at DESC);
