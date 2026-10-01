use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::{CatalogProduct, Content, Shop},
    AppState,
};

/// Card columns (product + supplier shop + listing-extension summaries as `ext`).
pub fn catalog_cols(st: &AppState) -> String {
    format!(
        "p.id, p.shop_id, p.sku, p.name, p.description, p.price_cents, p.images, p.category,
     p.stock, p.allow_resell, p.commission_bps, p.category_id, p.shop_category_id, p.cover, s.slug AS shop_slug, s.name AS shop_name, s.currency,
     (s.kyb_verified_at IS NOT NULL) AS shop_verified, {}",
        st.reg.catalog_ext_sql()
    )
}

/// SQL predicate: reseller shop `$via` may currently sell product `p`.
pub const RESELL_OK: &str = "EXISTS (
    SELECT 1 FROM listings l
    JOIN partnerships pa ON pa.reseller_shop_id = l.reseller_shop_id AND pa.supplier_shop_id = p.shop_id
    WHERE l.product_id = p.id AND l.reseller_shop_id = $via AND l.active AND pa.status = 'approved'
) AND p.allow_resell";

#[derive(Deserialize)]
pub struct CatalogQ {
    pub q: Option<String>,
    pub category: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list_products(
    State(st): State<AppState>,
    Query(q): Query<CatalogQ>,
) -> AppResult<Json<Vec<CatalogProduct>>> {
    let cols = catalog_cols(&st);
    let sql = format!(
        "SELECT {cols}, NULL::uuid AS via_shop_id
         FROM products p JOIN shops s ON s.id = p.shop_id
         WHERE p.status = 'active' AND p.review_status = 'approved'
           AND ($1::text IS NULL OR p.name ILIKE '%' || $1 || '%' OR p.description ILIKE '%' || $1 || '%')
           AND ($2::uuid[] IS NULL OR p.category_id = ANY($2))
         ORDER BY p.created_at DESC LIMIT $3 OFFSET $4"
    );
    // Category filter includes all sub-categories.
    let cat_ids: Option<Vec<Uuid>> = match q.category.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(slug) => Some(crate::routes::categories::subtree_ids_by_slug(&st, slug).await?),
        None => None,
    };
    let rows = sqlx::query_as(&sql)
        .bind(q.q.filter(|s| !s.trim().is_empty()))
        .bind(cat_ids)
        .bind(q.limit.unwrap_or(24).clamp(1, 100))
        .bind(q.offset.unwrap_or(0).max(0))
        .fetch_all(&st.db)
        .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct ViaQ {
    pub via: Option<Uuid>,
}

pub async fn get_product(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    Query(v): Query<ViaQ>,
) -> AppResult<Json<Value>> {
    let cols = catalog_cols(&st);
    let sql = format!(
        "SELECT {cols}, NULL::uuid AS via_shop_id
         FROM products p JOIN shops s ON s.id = p.shop_id WHERE p.id = $1 AND p.status = 'active' AND p.review_status = 'approved'"
    );
    let mut product: CatalogProduct = sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Resolve reseller storefront, if the visit came through one and it is still valid.
    let mut via_shop: Option<Shop> = None;
    if let Some(via) = v.via {
        let ok_sql = format!(
            "SELECT {} FROM products p WHERE p.id = $1",
            RESELL_OK.replace("$via", "$2")
        );
        let ok: bool = sqlx::query_scalar(&ok_sql)
            .bind(id)
            .bind(via)
            .fetch_one(&st.db)
            .await?;
        if ok {
            via_shop = sqlx::query_as("SELECT * FROM shops WHERE id = $1")
                .bind(via)
                .fetch_optional(&st.db)
                .await?;
            product.via_shop_id = via_shop.as_ref().map(|s| s.id);
        }
    }

    let contents: Vec<Content> = sqlx::query_as(
        "SELECT * FROM contents WHERE product_id = $1 AND status = 'published' ORDER BY created_at DESC LIMIT 6",
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;

    let media = crate::routes::media::public_gallery(&st, id).await?;
    // Breadcrumb: root → leaf marketplace category.
    let breadcrumb: Vec<(String, String)> = match product.category_id {
        Some(cid) => sqlx::query_as(
            "WITH RECURSIVE up AS (
                SELECT id, parent_id, name, slug, 0 AS d FROM categories WHERE id = $1
                UNION ALL SELECT c.id, c.parent_id, c.name, c.slug, up.d + 1 FROM categories c JOIN up ON c.id = up.parent_id
             ) SELECT name, slug FROM up ORDER BY d DESC",
        )
        .bind(cid)
        .fetch_all(&st.db)
        .await?,
        None => vec![],
    };
    let breadcrumb: Vec<Value> = breadcrumb.into_iter().map(|(name, slug)| json!({ "name": name, "slug": slug })).collect();
    // Listing extensions (vehicle spec sheet …) + who to call (the broker's storefront when visited through one).
    let mut ext = serde_json::Map::new();
    let mut show_contact = false;
    for e in &st.reg.listing {
        if let Some(d) = e.detail(&st, id).await? {
            show_contact |= e.show_contact();
            ext.insert(e.key().to_string(), d);
        }
    }
    let contact = if show_contact {
        let sid = via_shop.as_ref().map(|s| s.id).unwrap_or(product.shop_id);
        let c: Option<(String, String, String)> = sqlx::query_as("SELECT name, phone, address FROM shops WHERE id = $1")
            .bind(sid)
            .fetch_optional(&st.db)
            .await?;
        c.map(|(name, phone, address)| json!({ "name": name, "phone": phone, "address": address }))
    } else {
        None
    };
    let mut out = json!({ "product": product, "via_shop": via_shop, "contents": contents, "media": media,
                          "breadcrumb": breadcrumb, "contact": contact });
    for e in &st.reg.listing {
        out[e.key()] = ext.remove(e.key()).unwrap_or(Value::Null);
    }
    Ok(Json(out))
}

/// Public storefront: the shop's own products plus products it resells.
pub async fn storefront(
    State(st): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Value>> {
    let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE slug = $1")
        .bind(&slug)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let cols = catalog_cols(&st);
    let own_sql = format!(
        "SELECT {cols}, NULL::uuid AS via_shop_id
         FROM products p JOIN shops s ON s.id = p.shop_id
         WHERE p.shop_id = $1 AND p.status = 'active' AND p.review_status = 'approved' ORDER BY p.created_at DESC"
    );
    let own: Vec<CatalogProduct> = sqlx::query_as(&own_sql)
        .bind(shop.id)
        .fetch_all(&st.db)
        .await?;

    let cols = catalog_cols(&st);
    let resold_sql = format!(
        "SELECT {cols}, $1::uuid AS via_shop_id
         FROM products p JOIN shops s ON s.id = p.shop_id
         WHERE p.status = 'active' AND p.review_status = 'approved' AND {} ORDER BY p.name",
        RESELL_OK.replace("$via", "$1")
    );
    let resold: Vec<CatalogProduct> = sqlx::query_as(&resold_sql)
        .bind(shop.id)
        .fetch_all(&st.db)
        .await?;

    let contents: Vec<Content> = sqlx::query_as(
        "SELECT * FROM contents WHERE shop_id = $1 AND status='published' ORDER BY created_at DESC LIMIT 12",
    )
    .bind(shop.id)
    .fetch_all(&st.db)
    .await?;

    let categories = crate::routes::categories::load_tree(&st, Some(shop.id), true).await?;
    Ok(Json(
        json!({ "shop": shop, "products": own, "resold": resold, "contents": contents, "categories": categories }),
    ))
}
