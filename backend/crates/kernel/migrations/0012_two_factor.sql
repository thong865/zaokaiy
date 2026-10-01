-- Two-factor authentication: TOTP (authenticator app) + single-use recovery codes.
-- Secrets are encrypted at rest with the same AES-GCM key as KYC data (KYC_ENCRYPTION_KEY).

ALTER TABLE users ADD COLUMN totp_secret         TEXT;         -- sealed, set once confirmed
ALTER TABLE users ADD COLUMN totp_pending_secret TEXT;         -- sealed, between setup and confirm
ALTER TABLE users ADD COLUMN totp_enabled_at     TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN totp_last_step      BIGINT;       -- last accepted 30s step (no replays)
ALTER TABLE users ADD COLUMN totp_failures       INT NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN totp_locked_until   TIMESTAMPTZ;  -- set after too many wrong codes

-- Only a hash of each recovery code is kept.
CREATE TABLE user_recovery_codes (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash  TEXT NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX user_recovery_codes_user_idx ON user_recovery_codes (user_id);

-- A sign-in that passed the first factor and waits for the second (5 minutes).
CREATE TABLE mfa_challenges (
    token_hash  TEXT PRIMARY KEY,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    is_new      BOOLEAN NOT NULL DEFAULT false,
    redirect_to TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
