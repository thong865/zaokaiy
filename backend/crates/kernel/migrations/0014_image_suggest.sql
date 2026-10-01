-- Module: image_suggest — while a seller types a product name ("beer lao"), suggest photos that
-- already exist on the platform so they can pick one instead of shooting/uploading their own.
--
-- Sources, best first:
--   1. stock library: photos curated by admins (official brand shots, generic items), with keywords
--   2. the seller's own media library
--   3. product photos of shops that opted in to share them (image_share_optin)
-- Picking a stock/shared photo COPIES the files into the seller's library (their own media asset),
-- so later edits or deletions on either side never affect the other.
-- Switch the module off with IMAGE_SUGGEST_ENABLED=false; these tables are then just unused.

-- Text used for matching: lower case, spaces and punctuation removed ("Beer Lao 640 ml" → "beerlao640ml").
-- Lao/Thai text has no spaces anyway, so substring matching works the same for every language.
CREATE FUNCTION image_suggest_norm(t TEXT) RETURNS TEXT
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    AS $$ SELECT regexp_replace(lower(coalesce(t, '')), '[[:space:][:punct:]–—•·“”‘’…«»]+', '', 'g') $$;

CREATE TABLE stock_images (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title           TEXT NOT NULL,
    keywords        TEXT NOT NULL DEFAULT '',           -- comma separated, any language: "beerlao, ເບຍລາວ, เบียร์ลาว"
    brand           TEXT NOT NULL DEFAULT '',
    credit          TEXT NOT NULL DEFAULT '',           -- source / licence ("Lao Brewery Co. press kit")
    category_id     UUID REFERENCES categories(id) ON DELETE SET NULL,
    search_norm     TEXT NOT NULL DEFAULT '',
    mime            TEXT NOT NULL DEFAULT '',
    url             TEXT NOT NULL,
    thumb_url       TEXT,
    storage_key     TEXT,
    thumb_key       TEXT,
    size_bytes      BIGINT NOT NULL DEFAULT 0,
    width           INT,
    height          INT,
    variants        JSONB  NOT NULL DEFAULT '[]',
    variant_keys    TEXT[] NOT NULL DEFAULT '{}',
    placeholder     TEXT   NOT NULL DEFAULT '',
    dominant_color  TEXT   NOT NULL DEFAULT '',
    active          BOOLEAN NOT NULL DEFAULT true,
    use_count       INT NOT NULL DEFAULT 0,
    source_asset_id UUID REFERENCES media_assets(id) ON DELETE SET NULL,  -- promoted from this shop photo
    created_by      UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX stock_images_active_idx ON stock_images (created_at DESC) WHERE active;

-- Words that mean the same product: typing any of them finds photos tagged with the others.
CREATE TABLE image_suggest_synonyms (
    id         BIGSERIAL PRIMARY KEY,
    terms      TEXT[] NOT NULL,                        -- normalised (image_suggest_norm)
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO image_suggest_synonyms (terms) VALUES
    (ARRAY['beerlao', 'ເບຍລາວ', 'เบียร์ลาว', 'bialao']),
    (ARRAY['cocacola', 'coke', 'ໂຄກ', 'ໂຄຄາໂຄລາ', 'โค้ก']),
    (ARRAY['pepsi', 'ເປັບຊີ', 'เป๊ปซี่']),
    (ARRAY['tigerbeer', 'ເບຍໄທເກີ', 'เบียร์ไทเกอร์']),
    (ARRAY['heineken', 'ໄຮເນເກັນ', 'ไฮเนเก้น']),
    (ARRAY['laocoffee', 'ກາເຟລາວ', 'กาแฟลาว']),
    (ARRAY['stickyrice', 'ເຂົ້າໜຽວ', 'ข้าวเหนียว']),
    (ARRAY['sinh', 'ສິ້ນ', 'ผ้าซิ่น']);

-- Shops that let other sellers reuse their (approved, live) product photos.
CREATE TABLE image_share_optin (
    shop_id    UUID PRIMARY KEY REFERENCES shops(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Every pick: which photo a shop copied (dedupes repeat picks, credits the source, feeds popularity).
CREATE TABLE image_suggest_picks (
    id         BIGSERIAL PRIMARY KEY,
    shop_id    UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    source     TEXT NOT NULL CHECK (source IN ('stock','shared')),
    source_id  UUID NOT NULL,                          -- stock_images.id or media_assets.id
    asset_id   UUID REFERENCES media_assets(id) ON DELETE SET NULL,  -- the copy in the shop's library (NULL once deleted; history kept)
    query      TEXT NOT NULL DEFAULT '',
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX image_suggest_picks_shop_idx ON image_suggest_picks (shop_id, source, source_id);
CREATE INDEX image_suggest_picks_asset_idx ON image_suggest_picks (asset_id);
