-- Vehicle sellers: car / motorbike dealers and private sellers.
--   shops.vertical = 'vehicle' switches the storefront to a vehicle showroom (search by make,
--   model, year, price, mileage…), listings get a spec sheet and buyers send enquiries,
--   test-drive requests, offers or reservations (deposit) instead of — or next to — the cart.

ALTER TABLE shops
    ADD COLUMN vertical TEXT NOT NULL DEFAULT 'general' CHECK (vertical IN ('general','vehicle'));

CREATE TABLE vehicle_specs (
    product_id        UUID PRIMARY KEY REFERENCES products(id) ON DELETE CASCADE,
    vehicle_type      TEXT NOT NULL DEFAULT 'car'
                      CHECK (vehicle_type IN ('car','motorbike','truck','van','other')),
    condition         TEXT NOT NULL DEFAULT 'used' CHECK (condition IN ('new','used','reconditioned')),
    make              TEXT NOT NULL,
    model             TEXT NOT NULL,
    variant           TEXT NOT NULL DEFAULT '',
    year              INT  NOT NULL CHECK (year BETWEEN 1950 AND 2100),
    mileage_km        INT  NOT NULL DEFAULT 0 CHECK (mileage_km >= 0),
    fuel              TEXT NOT NULL DEFAULT 'petrol'
                      CHECK (fuel IN ('petrol','diesel','hybrid','electric','lpg','other')),
    transmission      TEXT NOT NULL DEFAULT 'automatic'
                      CHECK (transmission IN ('manual','automatic','cvt','semi_auto')),
    body_type         TEXT NOT NULL DEFAULT '',
    drive             TEXT NOT NULL DEFAULT '' CHECK (drive IN ('','fwd','rwd','4wd','awd')),
    engine_cc         INT  CHECK (engine_cc BETWEEN 0 AND 20000),
    power_hp          INT  CHECK (power_hp BETWEEN 0 AND 3000),
    seats             INT  CHECK (seats BETWEEN 1 AND 60),
    doors             INT  CHECK (doors BETWEEN 0 AND 10),
    color             TEXT NOT NULL DEFAULT '',
    owners            INT  CHECK (owners BETWEEN 0 AND 50),
    registered        BOOLEAN NOT NULL DEFAULT true,
    plate_province    TEXT NOT NULL DEFAULT '',
    -- private to the seller (the public API only exposes the last 4 VIN characters)
    plate_no          TEXT NOT NULL DEFAULT '',
    vin               TEXT NOT NULL DEFAULT '',
    location          TEXT NOT NULL DEFAULT '',
    features          TEXT[] NOT NULL DEFAULT '{}',
    warranty_months   INT  CHECK (warranty_months BETWEEN 0 AND 240),
    -- selling terms
    deposit_cents     BIGINT NOT NULL DEFAULT 0 CHECK (deposit_cents >= 0),
    negotiable        BOOLEAN NOT NULL DEFAULT true,
    finance_available BOOLEAN NOT NULL DEFAULT false,
    buy_online        BOOLEAN NOT NULL DEFAULT false,
    sale_status       TEXT NOT NULL DEFAULT 'available' CHECK (sale_status IN ('available','reserved','sold')),
    reserved_until    TIMESTAMPTZ,
    sold_at           TIMESTAMPTZ,
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX vehicle_specs_make_idx ON vehicle_specs (lower(make), lower(model));
CREATE INDEX vehicle_specs_type_idx ON vehicle_specs (vehicle_type, sale_status);
CREATE INDEX vehicle_specs_year_idx ON vehicle_specs (year);

-- Buyer requests on a vehicle listing.
CREATE TABLE vehicle_leads (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id         UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    product_id      UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    user_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    kind            TEXT NOT NULL CHECK (kind IN ('enquiry','test_drive','offer','reserve','finance')),
    name            TEXT NOT NULL,
    phone           TEXT NOT NULL,
    message         TEXT NOT NULL DEFAULT '',
    preferred_at    TIMESTAMPTZ,
    offer_cents     BIGINT CHECK (offer_cents > 0),
    deposit_cents   BIGINT NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'new' CHECK (status IN ('new','contacted','scheduled','won','lost')),
    scheduled_at    TIMESTAMPTZ,
    deposit_paid_at TIMESTAMPTZ,
    seller_note     TEXT NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX vehicle_leads_shop_idx ON vehicle_leads (shop_id, status, created_at DESC);
CREATE INDEX vehicle_leads_phone_idx ON vehicle_leads (phone, created_at DESC);

-- Marketplace categories for vehicles.
WITH top AS (
    INSERT INTO categories (name, slug, position)
    SELECT 'Vehicles', 'vehicles', COALESCE(MAX(position), 0) + 1 FROM categories WHERE shop_id IS NULL AND parent_id IS NULL
    ON CONFLICT DO NOTHING
    RETURNING id
), parent AS (
    SELECT id FROM top
    UNION ALL SELECT id FROM categories WHERE shop_id IS NULL AND slug = 'vehicles'
    LIMIT 1
)
INSERT INTO categories (parent_id, name, slug, position)
SELECT parent.id, v.name, v.slug, v.pos FROM parent,
    (VALUES ('Cars', 'vehicles-cars', 1), ('Motorbikes', 'vehicles-motorbikes', 2),
            ('Trucks & Vans', 'vehicles-trucks', 3), ('Parts & Accessories', 'vehicles-parts', 4)) AS v(name, slug, pos)
ON CONFLICT DO NOTHING;

-- A vehicle bought online (stock reaches 0) is marked sold; a cancelled order that restocks it
-- makes it available again.
CREATE FUNCTION vehicle_stock_sync() RETURNS trigger AS $$
BEGIN
    IF NEW.stock = 0 AND OLD.stock > 0 THEN
        UPDATE vehicle_specs SET sale_status = 'sold', sold_at = now(), reserved_until = NULL, updated_at = now()
        WHERE product_id = NEW.id AND sale_status <> 'sold';
    ELSIF NEW.stock > 0 AND OLD.stock = 0 THEN
        UPDATE vehicle_specs SET sale_status = 'available', sold_at = NULL, updated_at = now()
        WHERE product_id = NEW.id AND sale_status = 'sold';
    END IF;
    RETURN NEW;
END $$ LANGUAGE plpgsql;

CREATE TRIGGER products_vehicle_stock AFTER UPDATE OF stock ON products
    FOR EACH ROW WHEN (OLD.stock IS DISTINCT FROM NEW.stock) EXECUTE FUNCTION vehicle_stock_sync();
