-- Fast product images: responsive WebP renditions, blurred placeholder and dominant colour per
-- asset, and a denormalised `products.cover` (first gallery image with its renditions) so catalog
-- cards load a ~20-40 KB 640px file instead of the full-size original.

ALTER TABLE media_assets
    ADD COLUMN variants       JSONB  NOT NULL DEFAULT '[]',   -- [{w, h, url}] WebP, largest first
    ADD COLUMN variant_keys   TEXT[] NOT NULL DEFAULT '{}',   -- storage keys of the renditions
    ADD COLUMN placeholder    TEXT   NOT NULL DEFAULT '',     -- data:image/jpeg;base64,… (~24px)
    ADD COLUMN dominant_color TEXT   NOT NULL DEFAULT '';     -- #rrggbb

ALTER TABLE products ADD COLUMN cover JSONB;

-- Older uploads get their renditions from a background job at API start; external-URL images
-- (no renditions) still get a cover right away.
UPDATE products p SET cover = (
    SELECT jsonb_build_object('url', a.url, 'thumb_url', a.thumb_url, 'width', a.width, 'height', a.height,
                              'variants', a.variants, 'placeholder', a.placeholder, 'color', a.dominant_color, 'alt', a.alt)
    FROM product_media pm JOIN media_assets a ON a.id = pm.asset_id
    WHERE pm.product_id = p.id AND a.kind = 'image' ORDER BY pm.position LIMIT 1);
