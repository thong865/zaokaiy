-- Media library: every shop owns a library of assets (uploaded or external URLs)
-- that can be reused across product galleries, creator content and ads.

CREATE TABLE media_assets (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id       UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL CHECK (kind IN ('image','video')),
    source        TEXT NOT NULL DEFAULT 'upload' CHECK (source IN ('upload','url')),
    mime          TEXT NOT NULL DEFAULT '',
    url           TEXT NOT NULL,
    thumb_url     TEXT,
    storage_key   TEXT,              -- object key for uploads (NULL for external URLs)
    thumb_key     TEXT,
    size_bytes    BIGINT NOT NULL DEFAULT 0,
    width         INT,
    height        INT,
    alt           TEXT NOT NULL DEFAULT '',
    original_name TEXT NOT NULL DEFAULT '',
    created_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX media_assets_shop_idx ON media_assets(shop_id, created_at DESC);

-- Ordered product gallery. position 0 = cover.
CREATE TABLE product_media (
    product_id UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    asset_id   UUID NOT NULL REFERENCES media_assets(id) ON DELETE CASCADE,
    position   INT  NOT NULL DEFAULT 0,
    PRIMARY KEY (product_id, asset_id)
);
CREATE INDEX product_media_asset_idx ON product_media(asset_id);

-- Backfill: turn existing products.images URLs into library assets + gallery rows.
WITH src AS (
    SELECT p.id AS product_id, p.shop_id, img.value #>> '{}' AS url, img.ordinality - 1 AS position
    FROM products p, jsonb_array_elements(p.images) WITH ORDINALITY AS img(value, ordinality)
    WHERE jsonb_typeof(p.images) = 'array'
), ins AS (
    INSERT INTO media_assets (shop_id, kind, source, url, thumb_url)
    SELECT DISTINCT shop_id, 'image', 'url', url, url FROM src
    RETURNING id, shop_id, url
)
INSERT INTO product_media (product_id, asset_id, position)
SELECT src.product_id, ins.id, src.position
FROM src JOIN ins ON ins.shop_id = src.shop_id AND ins.url = src.url
ON CONFLICT DO NOTHING;
