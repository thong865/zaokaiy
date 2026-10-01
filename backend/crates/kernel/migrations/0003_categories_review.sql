-- Categories (marketplace-wide + per-shop) and product review/approval workflow.

-- ---------------------------------------------------------------------------
-- Categories
--   shop_id IS NULL  -> marketplace category, managed by platform admins
--   shop_id = <shop> -> the seller's own storefront category
--   parent_id builds the tree (category -> sub-category -> …, max depth 3)
-- ---------------------------------------------------------------------------
CREATE TABLE categories (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id     UUID REFERENCES shops(id) ON DELETE CASCADE,
    parent_id   UUID REFERENCES categories(id) ON DELETE RESTRICT,
    name        TEXT NOT NULL,
    slug        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    image_url   TEXT,
    position    INT  NOT NULL DEFAULT 0,
    active      BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- slug unique within its scope (marketplace or one shop)
CREATE UNIQUE INDEX categories_scope_slug_idx
    ON categories (COALESCE(shop_id, '00000000-0000-0000-0000-000000000000'::uuid), slug);
CREATE INDEX categories_parent_idx ON categories(parent_id);
CREATE INDEX categories_shop_idx ON categories(shop_id);

-- Starter marketplace taxonomy (admins can edit/extend it).
WITH top(name, slug, pos) AS (VALUES
    ('Fashion', 'fashion', 1), ('Beauty', 'beauty', 2), ('Home & Living', 'home', 3),
    ('Electronics', 'electronics', 4), ('Food & Drinks', 'food', 5), ('Handmade & Crafts', 'handmade', 6),
    ('Health & Wellness', 'health', 7), ('Kids & Baby', 'kids', 8)
), ins AS (
    INSERT INTO categories (name, slug, position) SELECT name, slug, pos FROM top RETURNING id, slug
), sub(parent, name, slug, pos) AS (VALUES
    ('fashion', 'Women', 'fashion-women', 1), ('fashion', 'Men', 'fashion-men', 2), ('fashion', 'Bags & Accessories', 'fashion-accessories', 3),
    ('beauty', 'Skincare', 'beauty-skincare', 1), ('beauty', 'Makeup', 'beauty-makeup', 2), ('beauty', 'Hair & Body', 'beauty-hair-body', 3),
    ('home', 'Kitchen & Dining', 'home-kitchen', 1), ('home', 'Decor', 'home-decor', 2), ('home', 'Textiles', 'home-textiles', 3),
    ('electronics', 'Phones & Tablets', 'electronics-phones', 1), ('electronics', 'Audio', 'electronics-audio', 2), ('electronics', 'Accessories', 'electronics-accessories', 3),
    ('food', 'Snacks', 'food-snacks', 1), ('food', 'Coffee & Tea', 'food-coffee-tea', 2), ('food', 'Local Specialties', 'food-local', 3),
    ('handmade', 'Ceramics', 'handmade-ceramics', 1), ('handmade', 'Woodwork', 'handmade-woodwork', 2), ('handmade', 'Weaving & Textiles', 'handmade-weaving', 3),
    ('health', 'Supplements', 'health-supplements', 1), ('health', 'Fitness', 'health-fitness', 2),
    ('kids', 'Toys', 'kids-toys', 1), ('kids', 'Baby Care', 'kids-baby-care', 2)
)
INSERT INTO categories (parent_id, name, slug, position)
SELECT ins.id, sub.name, sub.slug, sub.pos FROM sub JOIN ins ON ins.slug = sub.parent;

-- ---------------------------------------------------------------------------
-- Products: category links + review state
-- ---------------------------------------------------------------------------
ALTER TABLE products
    ADD COLUMN category_id      UUID REFERENCES categories(id) ON DELETE SET NULL,
    ADD COLUMN shop_category_id UUID REFERENCES categories(id) ON DELETE SET NULL,
    -- not_submitted: never sent for review (drafts) · pending: waiting for an admin
    -- approved: visible when status = 'active' · rejected: seller must fix & resubmit
    ADD COLUMN review_status    TEXT NOT NULL DEFAULT 'not_submitted'
        CHECK (review_status IN ('not_submitted','pending','approved','rejected')),
    ADD COLUMN review_note      TEXT NOT NULL DEFAULT '',
    ADD COLUMN submitted_at     TIMESTAMPTZ,
    ADD COLUMN reviewed_at      TIMESTAMPTZ,
    ADD COLUMN reviewed_by      UUID REFERENCES users(id) ON DELETE SET NULL;
CREATE INDEX products_review_idx ON products(review_status, submitted_at);
CREATE INDEX products_category_idx ON products(category_id);
CREATE INDEX products_shop_category_idx ON products(shop_category_id);

-- Backfill categories from the old free-text column.
UPDATE products p SET category_id = c.id
FROM categories c
WHERE c.shop_id IS NULL AND c.slug = lower(p.category);
UPDATE products p SET category_id = c.id
FROM categories c
WHERE p.category_id IS NULL AND c.shop_id IS NULL AND c.slug = 'home-textiles' AND lower(p.category) = 'textiles';

-- Existing live products are grandfathered as approved.
UPDATE products SET review_status = 'approved', reviewed_at = now()
WHERE status = 'active';

-- Trusted shops skip the queue.
ALTER TABLE shops ADD COLUMN auto_approve BOOLEAN NOT NULL DEFAULT false;

-- Audit log of every submission / decision.
CREATE TABLE product_reviews (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    actor_id   UUID REFERENCES users(id) ON DELETE SET NULL,
    action     TEXT NOT NULL CHECK (action IN ('submitted','approved','rejected','auto_approved','resubmitted')),
    note       TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX product_reviews_product_idx ON product_reviews(product_id, created_at DESC);
