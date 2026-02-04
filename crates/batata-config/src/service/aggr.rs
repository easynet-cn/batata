//! Aggregate configuration service
//!
//! Provides operations for managing aggregate configurations (datumId-based configs).
//!
//! Aggregate configurations allow grouping multiple configuration items under a
//! single parent dataId, where each item is identified by a unique datumId.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};

use batata_persistence::entity::config_info_aggr;

/// Aggregate configuration item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggrConfigItem {
    pub id: Option<u64>,
    pub data_id: String,
    pub group_id: String,
    pub datum_id: String,
    pub content: String,
    pub md5: Option<String>,
    pub tenant_id: String,
    pub app_name: Option<String>,
    pub gmt_create: Option<chrono::NaiveDateTime>,
    pub gmt_modified: Option<chrono::NaiveDateTime>,
}

/// Aggregate configuration page
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggrConfigPage {
    pub total_count: u64,
    pub page_number: u32,
    pub pages_available: u64,
    pub page_items: Vec<AggrConfigItem>,
}

/// Publish or update an aggregate configuration item
pub async fn publish_aggr(
    db: &DatabaseConnection,
    data_id: &str,
    group_id: &str,
    tenant_id: &str,
    datum_id: &str,
    content: &str,
    app_name: Option<&str>,
) -> anyhow::Result<bool> {
    let now = chrono::Utc::now().naive_utc();
    let md5 = format!("{:x}", md5::compute(content));

    // Check if already exists
    let existing = config_info_aggr::Entity::find()
        .filter(config_info_aggr::Column::DataId.eq(data_id))
        .filter(config_info_aggr::Column::GroupId.eq(group_id))
        .filter(config_info_aggr::Column::TenantId.eq(tenant_id))
        .filter(config_info_aggr::Column::DatumId.eq(datum_id))
        .one(db)
        .await?;

    if let Some(model) = existing {
        // Update existing
        let mut active: config_info_aggr::ActiveModel = model.into();
        active.content = Set(content.to_string());
        active.md5 = Set(Some(md5));
        active.gmt_modified = Set(now);
        if let Some(app) = app_name {
            active.app_name = Set(Some(app.to_string()));
        }
        active.update(db).await?;
    } else {
        // Insert new
        let active = config_info_aggr::ActiveModel {
            data_id: Set(data_id.to_string()),
            group_id: Set(group_id.to_string()),
            tenant_id: Set(tenant_id.to_string()),
            datum_id: Set(datum_id.to_string()),
            content: Set(content.to_string()),
            md5: Set(Some(md5)),
            app_name: Set(app_name.map(|s| s.to_string())),
            gmt_create: Set(now),
            gmt_modified: Set(now),
            ..Default::default()
        };
        active.insert(db).await?;
    }

    Ok(true)
}

/// Remove an aggregate configuration item
pub async fn remove_aggr(
    db: &DatabaseConnection,
    data_id: &str,
    group_id: &str,
    tenant_id: &str,
    datum_id: &str,
) -> anyhow::Result<bool> {
    let result = config_info_aggr::Entity::delete_many()
        .filter(config_info_aggr::Column::DataId.eq(data_id))
        .filter(config_info_aggr::Column::GroupId.eq(group_id))
        .filter(config_info_aggr::Column::TenantId.eq(tenant_id))
        .filter(config_info_aggr::Column::DatumId.eq(datum_id))
        .exec(db)
        .await?;

    Ok(result.rows_affected > 0)
}

/// Remove all aggregate configuration items for a parent config
pub async fn remove_all_aggr(
    db: &DatabaseConnection,
    data_id: &str,
    group_id: &str,
    tenant_id: &str,
) -> anyhow::Result<u64> {
    let result = config_info_aggr::Entity::delete_many()
        .filter(config_info_aggr::Column::DataId.eq(data_id))
        .filter(config_info_aggr::Column::GroupId.eq(group_id))
        .filter(config_info_aggr::Column::TenantId.eq(tenant_id))
        .exec(db)
        .await?;

    Ok(result.rows_affected)
}

/// Get a single aggregate configuration item
pub async fn get_aggr(
    db: &DatabaseConnection,
    data_id: &str,
    group_id: &str,
    tenant_id: &str,
    datum_id: &str,
) -> anyhow::Result<Option<AggrConfigItem>> {
    let model = config_info_aggr::Entity::find()
        .filter(config_info_aggr::Column::DataId.eq(data_id))
        .filter(config_info_aggr::Column::GroupId.eq(group_id))
        .filter(config_info_aggr::Column::TenantId.eq(tenant_id))
        .filter(config_info_aggr::Column::DatumId.eq(datum_id))
        .one(db)
        .await?;

    Ok(model.map(|m| AggrConfigItem {
        id: Some(m.id),
        data_id: m.data_id,
        group_id: m.group_id,
        datum_id: m.datum_id,
        content: m.content,
        md5: m.md5,
        tenant_id: m.tenant_id,
        app_name: m.app_name,
        gmt_create: Some(m.gmt_create),
        gmt_modified: Some(m.gmt_modified),
    }))
}

/// List all aggregate configuration items for a parent config
pub async fn list_aggr(
    db: &DatabaseConnection,
    data_id: &str,
    group_id: &str,
    tenant_id: &str,
) -> anyhow::Result<Vec<AggrConfigItem>> {
    let models = config_info_aggr::Entity::find()
        .filter(config_info_aggr::Column::DataId.eq(data_id))
        .filter(config_info_aggr::Column::GroupId.eq(group_id))
        .filter(config_info_aggr::Column::TenantId.eq(tenant_id))
        .order_by(config_info_aggr::Column::DatumId, Order::Asc)
        .all(db)
        .await?;

    Ok(models
        .into_iter()
        .map(|m| AggrConfigItem {
            id: Some(m.id),
            data_id: m.data_id,
            group_id: m.group_id,
            datum_id: m.datum_id,
            content: m.content,
            md5: m.md5,
            tenant_id: m.tenant_id,
            app_name: m.app_name,
            gmt_create: Some(m.gmt_create),
            gmt_modified: Some(m.gmt_modified),
        })
        .collect())
}

/// List aggregate configuration items with pagination
pub async fn list_aggr_page(
    db: &DatabaseConnection,
    data_id: &str,
    group_id: &str,
    tenant_id: &str,
    page_number: u32,
    page_size: u32,
) -> anyhow::Result<AggrConfigPage> {
    let mut query = config_info_aggr::Entity::find()
        .filter(config_info_aggr::Column::DataId.eq(data_id))
        .filter(config_info_aggr::Column::GroupId.eq(group_id))
        .filter(config_info_aggr::Column::TenantId.eq(tenant_id));

    // Get total count
    let total_count = query.clone().count(db).await?;

    // Calculate pagination
    let pages_available = total_count.div_ceil(page_size as u64);
    let offset = (page_number.saturating_sub(1)) * page_size;

    // Get page items
    query = query.order_by(config_info_aggr::Column::DatumId, Order::Asc);
    let models = query
        .offset(offset as u64)
        .limit(page_size as u64)
        .all(db)
        .await?;

    let page_items: Vec<AggrConfigItem> = models
        .into_iter()
        .map(|m| AggrConfigItem {
            id: Some(m.id),
            data_id: m.data_id,
            group_id: m.group_id,
            datum_id: m.datum_id,
            content: m.content,
            md5: m.md5,
            tenant_id: m.tenant_id,
            app_name: m.app_name,
            gmt_create: Some(m.gmt_create),
            gmt_modified: Some(m.gmt_modified),
        })
        .collect();

    Ok(AggrConfigPage {
        total_count,
        page_number,
        pages_available,
        page_items,
    })
}

/// Get the merged content of all aggregate configuration items
/// Returns all datum contents concatenated with newlines
pub async fn get_merged_aggr(
    db: &DatabaseConnection,
    data_id: &str,
    group_id: &str,
    tenant_id: &str,
) -> anyhow::Result<Option<String>> {
    let items = list_aggr(db, data_id, group_id, tenant_id).await?;

    if items.is_empty() {
        return Ok(None);
    }

    let merged = items
        .into_iter()
        .map(|item| item.content)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(Some(merged))
}

/// Count aggregate configuration items for a parent config
pub async fn count_aggr(
    db: &DatabaseConnection,
    data_id: &str,
    group_id: &str,
    tenant_id: &str,
) -> anyhow::Result<u64> {
    let count = config_info_aggr::Entity::find()
        .filter(config_info_aggr::Column::DataId.eq(data_id))
        .filter(config_info_aggr::Column::GroupId.eq(group_id))
        .filter(config_info_aggr::Column::TenantId.eq(tenant_id))
        .count(db)
        .await?;

    Ok(count)
}

/// List all datumIds for a parent config
pub async fn list_datum_ids(
    db: &DatabaseConnection,
    data_id: &str,
    group_id: &str,
    tenant_id: &str,
) -> anyhow::Result<Vec<String>> {
    let models = config_info_aggr::Entity::find()
        .filter(config_info_aggr::Column::DataId.eq(data_id))
        .filter(config_info_aggr::Column::GroupId.eq(group_id))
        .filter(config_info_aggr::Column::TenantId.eq(tenant_id))
        .order_by(config_info_aggr::Column::DatumId, Order::Asc)
        .all(db)
        .await?;

    Ok(models.into_iter().map(|m| m.datum_id).collect())
}

/// Batch publish aggregate configuration items
pub async fn batch_publish_aggr(
    db: &DatabaseConnection,
    data_id: &str,
    group_id: &str,
    tenant_id: &str,
    items: Vec<(String, String)>, // (datum_id, content)
    app_name: Option<&str>,
) -> anyhow::Result<u32> {
    let mut count = 0;
    for (datum_id, content) in items {
        if publish_aggr(db, data_id, group_id, tenant_id, &datum_id, &content, app_name).await? {
            count += 1;
        }
    }
    Ok(count)
}

/// Search aggregate configurations by dataId pattern
pub async fn search_aggr_by_data_id(
    db: &DatabaseConnection,
    data_id_pattern: &str,
    tenant_id: &str,
    page_number: u32,
    page_size: u32,
) -> anyhow::Result<AggrConfigPage> {
    let mut query = config_info_aggr::Entity::find()
        .filter(config_info_aggr::Column::DataId.like(format!("%{}%", data_id_pattern)))
        .filter(config_info_aggr::Column::TenantId.eq(tenant_id));

    // Get total count
    let total_count = query.clone().count(db).await?;

    // Calculate pagination
    let pages_available = total_count.div_ceil(page_size as u64);
    let offset = (page_number.saturating_sub(1)) * page_size;

    // Get page items
    query = query
        .order_by(config_info_aggr::Column::DataId, Order::Asc)
        .order_by(config_info_aggr::Column::DatumId, Order::Asc);
    let models = query
        .offset(offset as u64)
        .limit(page_size as u64)
        .all(db)
        .await?;

    let page_items: Vec<AggrConfigItem> = models
        .into_iter()
        .map(|m| AggrConfigItem {
            id: Some(m.id),
            data_id: m.data_id,
            group_id: m.group_id,
            datum_id: m.datum_id,
            content: m.content,
            md5: m.md5,
            tenant_id: m.tenant_id,
            app_name: m.app_name,
            gmt_create: Some(m.gmt_create),
            gmt_modified: Some(m.gmt_modified),
        })
        .collect();

    Ok(AggrConfigPage {
        total_count,
        page_number,
        pages_available,
        page_items,
    })
}
