use crate::config::AppConfig;
use crate::models::search::Pagination;
use serde_json::{json, Value};
use tracing::info;

pub fn build_profiles_catalog(
    items: Vec<Value>,
    config: &AppConfig,
    page: u32,
    limit: u32,
    total: i64,
) -> Value {
    info!("Page: {}, Limit: {}, Total: {}", page, limit, total);
    json!({
        "catalog": {
            "providers": [
                {
                    "id": config.bpp.id,
                    "descriptor": {
                        "name": config.bpp.catalog_name
                    },
                    "items": items
                }
            ]
        },
         "pagination": {
                "page": page,
                "limit": limit,
                "total": total
            }


    })
}

pub fn extract_pagination(message: &Value) -> Pagination {
    Pagination {
        page: message
            .get("pagination")
            .and_then(|p| p.get("page"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),

        limit: message
            .get("pagination")
            .and_then(|p| p.get("limit"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
    }
}
