-- Super app: shop types come from the enabled cores (general, vehicle, restaurant, insurance, …),
-- validated by the API instead of a fixed CHECK list.
ALTER TABLE shops DROP CONSTRAINT IF EXISTS shops_vertical_check;
