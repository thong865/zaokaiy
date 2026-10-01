//! Vehicle core: car / motorbike showroom, spec sheets, buyer requests (leads) and reservations.
//! Extends commerce listings through the registry — commerce itself knows nothing about vehicles.

pub use kernel::{auth, error, AppState};

pub mod vehicles;

use axum::routing::{get, patch, post};
use kernel::{config::Config, core::ListingExt, error::AppResult, BoxFut, CoreModule, Registry, Routes};
use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

pub fn module() -> CoreModule {
    CoreModule {
        verticals: &["vehicle"],
        install,
        lao_patterns: &[(r"^vehicle: (.*)$", "ຂໍ້ມູນລົດບໍ່ຖືກຕ້ອງ: $1")],
        ..CoreModule::new("vehicle", "Vehicles", "ລົດ", "car", routes)
    }
}

fn routes(_cfg: &Config) -> Routes {
    Routes::new()
        .route("/vehicles", get(vehicles::search))
        .route("/catalog/products/{id}/leads", post(vehicles::create_lead))
        .route("/products/{id}/vehicle", get(vehicles::get_vehicle).put(vehicles::put_vehicle))
        .route("/products/{id}/vehicle/status", post(vehicles::set_sale_status))
        .route("/shops/{id}/leads", get(vehicles::list_leads))
        .route("/leads/{id}", patch(vehicles::update_lead))
}

fn install(reg: &mut Registry) {
    reg.add_listing(VehicleListing);
}

/// `vehicle` on products: spec sheet in, card summary + public spec sheet out.
struct VehicleListing;

impl ListingExt for VehicleListing {
    fn key(&self) -> &'static str {
        "vehicle"
    }
    fn card_sql(&self) -> &'static str {
        "SELECT jsonb_build_object('vehicle_type', v.vehicle_type, 'make', v.make, 'model', v.model, 'year', v.year,
             'mileage_km', v.mileage_km, 'fuel', v.fuel, 'transmission', v.transmission, 'condition', v.condition,
             'buy_online', v.buy_online, 'sale_status', v.sale_status)
         FROM vehicle_specs v WHERE v.product_id = p.id"
    }
    fn clean(&self, input: Value) -> AppResult<Value> {
        let v: vehicles::VehicleInput =
            serde_json::from_value(input).map_err(|e| error::AppError::bad(format!("vehicle: {e}")))?;
        Ok(serde_json::to_value(v.clean()?).unwrap_or(Value::Null))
    }
    fn save<'a>(&'a self, conn: &'a mut PgConnection, product_id: Uuid, input: &'a Value) -> BoxFut<'a, AppResult<()>> {
        Box::pin(async move {
            let v: vehicles::VehicleInput =
                serde_json::from_value(input.clone()).map_err(|e| error::AppError::bad(format!("vehicle: {e}")))?;
            vehicles::upsert(conn, product_id, &v).await
        })
    }
    fn detail<'a>(&'a self, st: &'a AppState, product_id: Uuid) -> BoxFut<'a, AppResult<Option<Value>>> {
        Box::pin(vehicles::public_spec(st, product_id))
    }
    fn blocks_checkout<'a>(&'a self, conn: &'a mut PgConnection, product_ids: &'a [Uuid]) -> BoxFut<'a, AppResult<Option<String>>> {
        Box::pin(async move {
            Ok(sqlx::query_scalar(
                "SELECT p.name FROM vehicle_specs v JOIN products p ON p.id = v.product_id
                 WHERE v.product_id = ANY($1) AND (NOT v.buy_online OR v.sale_status <> 'available') LIMIT 1",
            )
            .bind(product_ids)
            .fetch_optional(conn)
            .await?)
        })
    }
    fn show_contact(&self) -> bool {
        true
    }
}
