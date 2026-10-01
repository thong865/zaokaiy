-- Shop staff with roles, and POS open bills.
--
-- Staff: the shop owner invites people (a one-time link) and gives each a role. A role is a named
-- set of permissions (products, inventory, media, orders, delivery, pos, pos_void, social, leads,
-- marketing, network, stats, settings). Staff management, business verification and deleting the
-- shop stay with the owner. Every API route is mapped to one permission (backend/src/access.rs).

CREATE TABLE shop_roles (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id     UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    name        TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
    permissions TEXT[] NOT NULL DEFAULT '{}',
    template    TEXT,                          -- manager | cashier | stock | orders | marketing (built-in starters)
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (shop_id, name)
);

CREATE TABLE shop_members (
    shop_id      UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id      UUID NOT NULL REFERENCES shop_roles(id) ON DELETE RESTRICT,
    active       BOOLEAN NOT NULL DEFAULT true,  -- false = suspended (keeps history, no access)
    invited_by   UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ,
    PRIMARY KEY (shop_id, user_id)
);
CREATE INDEX shop_members_user_idx ON shop_members (user_id) WHERE active;

-- One-time invite links. Only a hash of the token is stored.
CREATE TABLE shop_invites (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id     UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    role_id     UUID NOT NULL REFERENCES shop_roles(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,
    note        TEXT NOT NULL DEFAULT '',        -- who it is for ("Noy — cashier, morning shift")
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at  TIMESTAMPTZ NOT NULL,
    accepted_by UUID REFERENCES users(id) ON DELETE SET NULL,
    accepted_at TIMESTAMPTZ,
    revoked_at  TIMESTAMPTZ
);
CREATE INDEX shop_invites_shop_idx ON shop_invites (shop_id, created_at DESC);

-- POS: several bills open at once (tables, customers still shopping, parked sales). Shared by all
-- tills of the shop; `version` rejects a stale save from another device.
CREATE TABLE pos_open_bills (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id     UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    number      INT NOT NULL,
    label       TEXT NOT NULL DEFAULT '',
    data        JSONB NOT NULL DEFAULT '{}',     -- the till's cart: lines, discount, note, customer
    version     INT NOT NULL DEFAULT 1,
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX pos_open_bills_shop_idx ON pos_open_bills (shop_id, number);
