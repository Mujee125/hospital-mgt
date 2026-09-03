//! Inventory — items and stock movements (CR-21, SRS FR-0180/0181/0185).
//!
//! Stock is stored as NUMERIC(14,2) and round-tripped via `rust_decimal`,
//! matching the billing pattern. All stock changes go through
//! `adjust_inventory`, which atomically updates `stock_quantity` and writes a
//! row to `inventory_movements` with the resulting balance snapshot. Direct
//! UPDATEs to `stock_quantity` outside this module are discouraged — they
//! bypass the movement audit trail.
//!
//! Every command is RBAC-guarded (`InventoryView` for reads, `InventoryManage`
//! for writes) and every write is audit-logged.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use sqlx::PgPool;

use crate::audit;
use crate::models::{CreateInventoryItem, InventoryItem, InventoryMovement, UpdateInventoryItem};
use crate::rbac::{self, Permission, SessionState};

fn dec(f: f64) -> Decimal {
    Decimal::from_f64(f).unwrap_or_default()
}

fn parse_date(s: &Option<String>) -> Result<Option<NaiveDate>, String> {
    match s {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(Some)
            .map_err(|_| "Invalid date format. Use YYYY-MM-DD.".to_string()),
    }
}

const SELECT_ITEMS: &str = r#"
    SELECT id, name, sku, category, unit, stock_quantity, reorder_level,
           expiry_date, batch_number, unit_cost, is_active, created_at, updated_at
    FROM inventory_items
"#;

#[tauri::command]
pub async fn get_inventory_items(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    category_filter: Option<String>,
    low_stock_only: Option<bool>,
) -> Result<Vec<InventoryItem>, String> {
    let _ = rbac::require(&session, Permission::InventoryView)?;

    let cat = category_filter.as_deref().filter(|s| !s.is_empty());
    let low = low_stock_only.unwrap_or(false);

    let q = match (cat, low) {
        (Some(_), true) => format!(
            "{} WHERE category = $1 AND stock_quantity <= reorder_level \
             ORDER BY name ASC",
            SELECT_ITEMS
        ),
        (Some(_), false) => format!("{} WHERE category = $1 ORDER BY name ASC", SELECT_ITEMS),
        (None, true) => format!(
            "{} WHERE stock_quantity <= reorder_level ORDER BY name ASC",
            SELECT_ITEMS
        ),
        (None, false) => format!("{} ORDER BY name ASC", SELECT_ITEMS),
    };

    let mut query = sqlx::query_as::<_, InventoryItem>(&q);
    if let Some(c) = cat {
        query = query.bind(c);
    }
    query
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get inventory items: {}", e))
}

#[tauri::command]
pub async fn get_inventory_item(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<InventoryItem, String> {
    let _ = rbac::require(&session, Permission::InventoryView)?;
    let q = format!("{} WHERE id = $1", SELECT_ITEMS);
    sqlx::query_as::<_, InventoryItem>(&q)
        .bind(id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| format!("Inventory item not found: {}", e))
}

#[tauri::command]
pub async fn create_inventory_item(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    item: CreateInventoryItem,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::InventoryManage)?;
    if item.name.trim().is_empty() {
        return Err("Item name is required.".to_string());
    }
    let expiry = parse_date(&item.expiry_date)?;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO inventory_items
              (name, sku, category, unit, stock_quantity, reorder_level,
               expiry_date, batch_number, unit_cost, is_active)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING id"#,
    )
    .bind(&item.name)
    .bind(&item.sku)
    .bind(item.category.as_deref().unwrap_or("medication"))
    .bind(&item.unit)
    .bind(dec(item.stock_quantity.unwrap_or(0.0)))
    .bind(dec(item.reorder_level.unwrap_or(0.0)))
    .bind(expiry)
    .bind(&item.batch_number)
    .bind(dec(item.unit_cost.unwrap_or(0.0)))
    .bind(item.is_active.unwrap_or(true))
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Create inventory item: {}", e))?;

    // If the item is created with an opening stock, record a movement so the
    // audit trail reflects the initial balance.
    let opening = dec(item.stock_quantity.unwrap_or(0.0));
    if opening != Decimal::ZERO {
        let _ = sqlx::query(
            r#"INSERT INTO inventory_movements
                  (item_id, quantity_change, reason, balance_after, created_by_user_id, notes)
               VALUES ($1, $2, 'initial_stock', $2, $3, 'Opening balance at item creation')"#,
        )
        .bind(row.0)
        .bind(opening)
        .bind(s.user_id)
        .execute(pool.inner())
        .await;
    }

    audit::for_session(
        pool.inner(),
        &s,
        "inventory_item_create",
        "inventory_items",
        Some(&row.0.to_string()),
        Some(serde_json::json!({
            "name": item.name,
            "sku": item.sku,
            "opening_stock": opening.to_string()
        })),
    )
    .await;
    Ok(row.0)
}

#[tauri::command]
pub async fn update_inventory_item(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
    item: UpdateInventoryItem,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::InventoryManage)?;
    if id != item.id {
        return Err("Path id does not match body id.".to_string());
    }
    if item.name.trim().is_empty() {
        return Err("Item name is required.".to_string());
    }
    let expiry = parse_date(&item.expiry_date)?;

    sqlx::query(
        r#"UPDATE inventory_items SET
              name=$1, sku=$2, category=$3, unit=$4,
              stock_quantity=$5, reorder_level=$6,
              expiry_date=$7, batch_number=$8,
              unit_cost=$9, is_active=$10, updated_at=NOW()
           WHERE id=$11"#,
    )
    .bind(&item.name)
    .bind(&item.sku)
    .bind(&item.category)
    .bind(&item.unit)
    .bind(dec(item.stock_quantity))
    .bind(dec(item.reorder_level))
    .bind(expiry)
    .bind(&item.batch_number)
    .bind(dec(item.unit_cost))
    .bind(item.is_active)
    .bind(id)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("Update inventory item: {}", e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "inventory_item_update",
        "inventory_items",
        Some(&id.to_string()),
        Some(serde_json::json!({"name": item.name, "is_active": item.is_active})),
    )
    .await;
    Ok(())
}

/// Adjust stock for an item by `quantity_change` (can be negative for
/// dispensing). Verifies the item exists, locks the row, updates
/// `stock_quantity`, and writes a movement row — all inside one transaction
/// so the balance and the audit trail can never drift apart. Refuses to
/// drive stock negative.
#[tauri::command]
pub async fn adjust_inventory(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    item_id: i32,
    quantity_change: i32,
    reason: String,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::InventoryManage)?;
    let reason_trimmed = reason.trim();
    if reason_trimmed.is_empty() {
        return Err("A reason is required for every stock adjustment.".to_string());
    }
    if quantity_change == 0 {
        return Err("Quantity change must be non-zero.".to_string());
    }

    let mut tx = pool.begin().await.map_err(|e| format!("Begin tx: {}", e))?;

    // Lock the row for the duration of the transaction so concurrent
    // adjustments cannot interleave and produce a wrong balance_after.
    let current: Option<(Decimal,)> =
        sqlx::query_as("SELECT stock_quantity FROM inventory_items WHERE id = $1 FOR UPDATE")
            .bind(item_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Lock inventory item: {}", e))?;

    let current_balance = current
        .ok_or_else(|| format!("Inventory item {} not found.", item_id))?
        .0;

    let change_dec = Decimal::from(quantity_change);
    let new_balance = current_balance + change_dec;
    if new_balance < Decimal::ZERO {
        return Err(format!(
            "Adjustment would drive stock negative (current: {}, change: {}). \
             Dispense cannot exceed available quantity.",
            current_balance, quantity_change
        ));
    }

    sqlx::query("UPDATE inventory_items SET stock_quantity = $1, updated_at = NOW() WHERE id = $2")
        .bind(new_balance)
        .bind(item_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Update stock_quantity: {}", e))?;

    sqlx::query(
        r#"INSERT INTO inventory_movements
              (item_id, quantity_change, reason, balance_after, created_by_user_id)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(item_id)
    .bind(change_dec)
    .bind(reason_trimmed)
    .bind(new_balance)
    .bind(s.user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Insert movement: {}", e))?;

    tx.commit().await.map_err(|e| format!("Commit: {}", e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "inventory_adjust",
        "inventory_items",
        Some(&item_id.to_string()),
        Some(serde_json::json!({
            "change": quantity_change,
            "reason": reason_trimmed,
            "balance_after": new_balance.to_string()
        })),
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn get_inventory_movements(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    item_id: Option<i32>,
    limit: Option<i64>,
) -> Result<Vec<InventoryMovement>, String> {
    let _ = rbac::require(&session, Permission::InventoryView)?;
    let limit = limit.unwrap_or(100).clamp(1, 5000);

    let rows = sqlx::query_as::<_, InventoryMovement>(
        r#"SELECT id, item_id, quantity_change, reason, balance_after,
                  reference_id, notes, created_by_user_id, created_at
           FROM inventory_movements
           WHERE ($1::int IS NULL OR item_id = $1)
           ORDER BY created_at DESC
           LIMIT $2"#,
    )
    .bind(item_id)
    .bind(limit)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("Get inventory movements: {}", e))?;

    Ok(rows)
}
