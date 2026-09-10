//! RCTF-FULL-SYSTEM-2026-09-08 F-16/F-17 — Pharmacy dispense stock matching
//! and inventory-edit integrity.
//!
//! F-16: dispense previously matched inventory by the prescription item's
//! FREE-TEXT medication_name; a no-match silently marked the item dispensed
//! with ZERO stock deduction and only an audit JSON field recording it —
//! stock drifted from reality with no movement row. Now: catalog identity
//! (medication_id → brand/generic) first, free-text fallback, and a no-match
//! dispense is REFUSED unless explicitly acknowledged as non-stock.
//!
//! F-17: update_inventory_item previously rewrote stock_quantity with no
//! movement row, no negative check, and no lock. Now: locked in-tx; a
//! quantity change writes a 'correction' movement; negative stock refused.
//!
//! Requires: HMS_TEST_DB_URL + `--features hms-integration-tests,server-build`.

#![cfg(all(feature = "hms-integration-tests", feature = "server-build"))]

mod common;

use common::*;
use hospital_mgmt_lib::commands::inventory::update_inventory_item_core;
use hospital_mgmt_lib::commands::pharmacy::dispense_prescription_item_core;
use hospital_mgmt_lib::models::UpdateInventoryItem;
use sqlx::PgPool;

// ── Fixtures ──────────────────────────────────────────────────────────────────

/// A pharmacist session (InventoryManage holder).
async fn pharm_session(pool: &PgPool, tag: &str) -> hospital_mgmt_lib::rbac::Session {
    let pw = fixture_pw();
    let uid = seed_user(pool, &format!("f16_{}", tag), &pw, &["pharmacist"]).await;
    let hash = format!("hash_f16_{}", tag);
    seed_session_row(pool, uid, &hash).await;
    load_session_for(pool, uid, &hash).await
}

/// A prescription item with an explicit catalog link (medication_id set).
async fn seed_catalog_rx_item(pool: &PgPool, patient_id: i32, brand: &str, generic: &str) -> i32 {
    let med: (i32,) = sqlx::query_as(
        "INSERT INTO medications (brand_name, generic_name, strength) \
         VALUES ($1, $2, '500mg') RETURNING id",
    )
    .bind(brand)
    .bind(generic)
    .fetch_one(pool)
    .await
    .expect("seed medication");
    let rx: (i32,) = sqlx::query_as(
        "INSERT INTO prescriptions (patient_id, status) VALUES ($1, 'active') RETURNING id",
    )
    .bind(patient_id)
    .fetch_one(pool)
    .await
    .expect("seed prescription");
    let item: (i32,) = sqlx::query_as(
        "INSERT INTO prescription_items \
         (prescription_id, medication_id, medication_name, dose, route, frequency, quantity) \
         VALUES ($1, $2, $3, '500mg', 'oral', 'BD', 10) RETURNING id",
    )
    .bind(rx.0)
    .bind(med.0)
    // The free-text name DELIBERATELY does not match the inventory name —
    // only the catalog link can find the stock row.
    .bind(format!("{} scribbled", brand))
    .fetch_one(pool)
    .await
    .expect("seed rx item");
    item.0
}

/// An inventory row with the given name and stock; returns its id.
async fn seed_inventory(pool: &PgPool, name: &str, stock: i64) -> i32 {
    let row: (i32,) = sqlx::query_as(
        "INSERT INTO inventory_items (name, stock_quantity) VALUES ($1, $2) RETURNING id",
    )
    .bind(name)
    .bind(stock)
    .fetch_one(pool)
    .await
    .expect("seed inventory");
    row.0
}

/// A free-text prescription item (no catalog link), like the nursing fixture.
async fn seed_freetext_rx_item(pool: &PgPool, patient_id: i32, med_name: &str) -> i32 {
    let rx: (i32,) = sqlx::query_as(
        "INSERT INTO prescriptions (patient_id, status) VALUES ($1, 'active') RETURNING id",
    )
    .bind(patient_id)
    .fetch_one(pool)
    .await
    .expect("seed prescription");
    let item: (i32,) = sqlx::query_as(
        "INSERT INTO prescription_items \
         (prescription_id, medication_id, medication_name, dose, route, frequency, quantity) \
         VALUES ($1, NULL, $2, '500mg', 'oral', 'BD', 10) RETURNING id",
    )
    .bind(rx.0)
    .bind(med_name)
    .fetch_one(pool)
    .await
    .expect("seed rx item");
    item.0
}

fn update_item(id: i32, name: &str, stock: f64) -> UpdateInventoryItem {
    UpdateInventoryItem {
        id,
        name: name.into(),
        sku: None,
        category: "medication".into(),
        unit: Some("tablet".into()),
        stock_quantity: stock,
        reorder_level: 10.0,
        expiry_date: None,
        batch_number: None,
        unit_cost: 0.0,
        is_active: true,
    }
}

// ── F-16: dispense stock matching ─────────────────────────────────────────────

/// Catalog identity finds the stock row even when the free-text name on
/// the prescription item does NOT match any inventory name — the old
/// free-text-only match would have found nothing and silently dispensed
/// with zero deduction.
#[tokio::test]
async fn rctf_f16_catalog_match_beats_freetext_mismatch() {
    let pool = test_pool().await;
    let s = pharm_session(&pool, "cat").await;
    let patient_id = seed_patient_with_phone(&pool, "Foa", "Catalog", "+92300f16a").await;

    // Inventory named by the CATALOG's brand name.
    let inv_id = seed_inventory(&pool, "Catalex", 100).await;
    // The rx item's free-text name says something else entirely.
    let item = seed_catalog_rx_item(&pool, patient_id, "Catalex", "cetirizine").await;

    dispense_prescription_item_core(&pool, &s, item, None)
        .await
        .expect("catalog-linked dispense must match inventory");

    // Stock WAS decremented and a movement row exists.
    let (stock,): (f64,) =
        sqlx::query_as("SELECT stock_quantity::float8 FROM inventory_items WHERE id = $1")
            .bind(inv_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stock, 90.0, "10 units dispensed from 100");
    let (movements,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM inventory_movements WHERE item_id = $1 AND reason = 'dispense'",
    )
    .bind(inv_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(movements, 1, "the dispense must be traceable");
}

/// A no-match dispense is REFUSED by default — the old behavior marked the
/// item dispensed with a silent zero deduction. With the explicit ack it
/// dispenses and records non_stock: true in the audit row.
#[tokio::test]
async fn rctf_f16_no_match_refused_unless_acknowledged() {
    let pool = test_pool().await;
    let s = pharm_session(&pool, "ns").await;
    let patient_id = seed_patient_with_phone(&pool, "Fob", "Nostock", "+92300f16b").await;

    // No inventory row matches this name in any form.
    let item = seed_freetext_rx_item(&pool, patient_id, "Unobtainium 500mg").await;

    let err = dispense_prescription_item_core(&pool, &s, item, None)
        .await
        .unwrap_err();
    assert!(
        err.contains("no matching inventory item"),
        "no-match dispense must be refused, got: {}",
        err
    );
    // The item is NOT dispensed.
    let (dispensed,): (bool,) =
        sqlx::query_as("SELECT dispensed FROM prescription_items WHERE id = $1")
            .bind(item)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!dispensed, "a refused dispense must not mark the item");

    // The acknowledged retry succeeds — recorded as non-stock.
    dispense_prescription_item_core(&pool, &s, item, Some(true))
        .await
        .expect("acknowledged non-stock dispense must succeed");
    let (dispensed,): (bool,) =
        sqlx::query_as("SELECT dispensed FROM prescription_items WHERE id = $1")
            .bind(item)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(dispensed);
    let (audit_non_stock,): (bool,) = sqlx::query_as(
        "SELECT (details->>'non_stock')::boolean FROM audit_logs \
         WHERE action = 'prescription_dispense' AND resource_id = $1 ORDER BY id DESC LIMIT 1",
    )
    .bind(item.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(audit_non_stock, "the audit row must record non_stock: true");
}

/// The legacy free-text match still works for items without a catalog link.
#[tokio::test]
async fn rctf_f16_freetext_fallback_still_matches() {
    let pool = test_pool().await;
    let s = pharm_session(&pool, "ft").await;
    let patient_id = seed_patient_with_phone(&pool, "Foc", "Freetext", "+92300f16c").await;

    let inv_id = seed_inventory(&pool, "panadol", 50).await;
    // Case differs from the inventory name — the legacy match is
    // case-insensitive.
    let item = seed_freetext_rx_item(&pool, patient_id, "Panadol").await;

    dispense_prescription_item_core(&pool, &s, item, None)
        .await
        .expect("free-text match must still work");
    let (stock,): (f64,) =
        sqlx::query_as("SELECT stock_quantity::float8 FROM inventory_items WHERE id = $1")
            .bind(inv_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stock, 40.0, "10 of 50 dispensed");
}

// ── F-17: inventory edit integrity ───────────────────────────────────────────

/// A stock change through the EDIT path now writes a 'correction'
/// movement row (the module invariant), and negative stock is refused.
#[tokio::test]
async fn rctf_f17_edit_records_correction_and_refuses_negative() {
    let pool = test_pool().await;
    let s = pharm_session(&pool, "cor").await;
    let inv_id = seed_inventory(&pool, "F17-Correction-Med", 100).await;

    // Change stock 100 → 70 via the edit path.
    update_inventory_item_core(
        &pool,
        &s,
        inv_id,
        update_item(inv_id, "F17-Correction-Med", 70.0),
    )
    .await
    .expect("edit with a stock change must succeed");

    let (stock,): (f64,) =
        sqlx::query_as("SELECT stock_quantity::float8 FROM inventory_items WHERE id = $1")
            .bind(inv_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stock, 70.0);
    let (change, movements): (f64, i64) = sqlx::query_as(
        "SELECT quantity_change::float8, COUNT(*)::bigint \
         FROM inventory_movements WHERE item_id = $1 AND reason = 'correction' GROUP BY 1",
    )
    .bind(inv_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        change, -30.0,
        "the edit must record the -30 correction movement"
    );
    assert_eq!(movements, 1);

    // A no-change edit writes NO correction movement (name-only edits stay
    // movement-free).
    update_inventory_item_core(
        &pool,
        &s,
        inv_id,
        update_item(inv_id, "F17-Correction-Med-Renamed", 70.0),
    )
    .await
    .expect("rename without a stock change must succeed");
    let (movements,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM inventory_movements WHERE item_id = $1 AND reason = 'correction'",
    )
    .bind(inv_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(movements, 1, "no additional movement for a no-change edit");

    // Negative stock through the edit path is refused outright.
    let err = update_inventory_item_core(
        &pool,
        &s,
        inv_id,
        update_item(inv_id, "F17-Correction-Med-Renamed", -5.0),
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("cannot be negative"),
        "negative stock must be refused, got: {}",
        err
    );
    // And nothing was written by the failed attempt.
    let (stock,): (f64,) =
        sqlx::query_as("SELECT stock_quantity::float8 FROM inventory_items WHERE id = $1")
            .bind(inv_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stock, 70.0, "the refused edit must change nothing");
}
