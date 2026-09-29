# VitalFlow HMS

A hospital management system for Windows, built as a desktop application.

## What it does
Patients, appointments and queue, laboratory, radiology, pharmacy,
inventory, blood bank, IPD/nursing, billing, reports, and internal messaging.

## Security
- Role-based access control and audit logging
- AES-256-GCM encrypted backups
- Ed25519-signed licensing and activation
- Single-use machine pairing for multi-PC setups

## Tech stack
Rust (Axum, SQLx) - Tauri 2 - PostgreSQL - React 19 - TypeScript - Tailwind CSS 4
Testing: Rust unit/integration tests, Vitest, Playwright

## Status
v0.4.0, in active development. 500+ automated tests.

## Documentation
Requirements, design, ISO-based quality/security/risk documents, and the
changelog are in the [docs](docs/) folder.
