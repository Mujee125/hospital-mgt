//! Tauri command modules. Each module groups commands for one clinical domain
//! and applies RBAC guards + audit logging uniformly.

pub mod appointments;
pub mod backup;
pub mod accounts;
pub mod billing;
pub mod blood_bank;
pub mod dashboard;
pub mod doctors;
pub mod encounters;
pub mod inventory;
pub mod ipd;
pub mod lab;
pub mod nursing;
pub mod patients;
pub mod pharmacy;
pub mod queue;
pub mod radiology;
pub mod reports;
pub mod system_health;
pub mod notifications;
pub mod search;
