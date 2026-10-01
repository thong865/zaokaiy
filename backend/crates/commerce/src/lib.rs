//! Commerce core: products and catalog, orders and checkout, the reseller (sell-staff) network,
//! delivery + COD, POS, social / live selling, ads, creator content and the AI assistant.

pub use kernel::{access, auth, config, crypto, error, i18n, media, storage, AppState};

pub mod ai;
pub mod models;
pub mod modules;
pub mod routes;
pub mod social_parser;
mod tax;

use std::time::Duration;

use kernel::{core::ShopHook, BoxFut, CoreModule, Registry};
use sqlx::PgConnection;
use uuid::Uuid;

pub fn module() -> CoreModule {
    CoreModule {
        verticals: &["general"],
        install,
        start,
        lao_fn: Some(modules::translate_lo),
        ..CoreModule::new("commerce", "Shop", "ຮ້ານຄ້າ", "shopping-bag", |cfg| routes::routes(cfg).merge(modules::router()))
    }
}

fn install(reg: &mut Registry) {
    reg.add_shop_hook(PauseProducts);
    reg.add_tax_source(tax::Orders);
    reg.add_tax_source(tax::SocialOrders);
    reg.add_tax_source(tax::PosSales);
}

/// Revoked business verification → products leave the marketplace until the shop is verified again.
struct PauseProducts;

impl ShopHook for PauseProducts {
    fn verification_revoked<'a>(&'a self, conn: &'a mut PgConnection, _st: &'a AppState, shop_id: Uuid) -> BoxFut<'a, error::AppResult<()>> {
        Box::pin(async move {
            sqlx::query(
                "UPDATE products SET review_status = 'not_submitted', review_note = $2, updated_at = now()
                 WHERE shop_id = $1 AND review_status IN ('approved','pending')",
            )
            .bind(shop_id)
            .bind("business verification revoked")
            .execute(conn)
            .await?;
            Ok(())
        })
    }
}

fn start(state: AppState) {
    // Optional plug-in modules (src/modules): background jobs of the enabled ones.
    modules::spawn_jobs(&state);
    // Expire unpaid social orders and release their stock.
    {
        let st = state.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(60));
            loop {
                tick.tick().await;
                match routes::social::expire_due(&st).await {
                    Ok(0) => {}
                    Ok(n) => tracing::info!(n, "expired social orders"),
                    Err(e) => tracing::warn!(error = %e, "social expiry job failed"),
                }
            }
        });
    }
    // Responsive renditions for images uploaded before they existed (batches of 50).
    tokio::spawn(async move {
        loop {
            match routes::media::backfill_variants(&state, 50).await {
                Ok((0, 0, _)) => break,
                Ok((ok, failed, remaining)) => {
                    tracing::info!(ok, failed, remaining, "media renditions backfill");
                    if remaining == 0 {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "media renditions backfill failed");
                    break;
                }
            }
        }
    });
}
