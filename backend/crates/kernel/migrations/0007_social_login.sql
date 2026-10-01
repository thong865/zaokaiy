-- Social login: Google, Facebook (OAuth 2.0) and WhatsApp (one-time code).
-- Accounts may now exist without an email (WhatsApp) or without a password (social only).

ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;
ALTER TABLE users ADD COLUMN phone TEXT UNIQUE;          -- E.164, e.g. +8562055551234
ALTER TABLE users ADD COLUMN avatar_url TEXT;

-- A login method linked to a user. subject = provider's stable user id (Google "sub",
-- Facebook app-scoped id) or the E.164 phone number for WhatsApp.
CREATE TABLE user_identities (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider      TEXT NOT NULL CHECK (provider IN ('google','facebook','whatsapp')),
    subject       TEXT NOT NULL,
    email         TEXT,
    name          TEXT NOT NULL DEFAULT '',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (provider, subject),
    UNIQUE (user_id, provider)
);

-- OAuth "state" for an authorization request in flight (10 minutes). Holds the PKCE
-- verifier, where to send the browser afterwards, and the user when linking an account.
CREATE TABLE oauth_states (
    state         TEXT PRIMARY KEY,
    provider      TEXT NOT NULL,
    code_verifier TEXT NOT NULL,
    redirect_to   TEXT NOT NULL DEFAULT '/',
    link_user_id  UUID REFERENCES users(id) ON DELETE CASCADE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One-time codes handed to the web app after an OAuth callback (2 minutes), exchanged for a
-- session token by POST /auth/exchange so the token itself never appears in a URL.
CREATE TABLE login_codes (
    code_hash    TEXT PRIMARY KEY,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    is_new       BOOLEAN NOT NULL DEFAULT false,
    linked       TEXT,
    redirect_to  TEXT NOT NULL DEFAULT '/',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- WhatsApp one-time passwords (10 minutes, 5 attempts). Only a hash of the code is kept.
CREATE TABLE phone_otps (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    phone       TEXT NOT NULL,
    code_hash   TEXT NOT NULL,
    attempts    INT NOT NULL DEFAULT 0,
    consumed_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX phone_otps_phone_idx ON phone_otps (phone, created_at DESC);
