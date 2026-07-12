# VitalFlow HMS — Documentation

This folder is the authoritative source for all project documentation per the RCTF prompt. The Phase 1 audit (DOC-01) found that this folder did not exist; Batch 4 created it and reconciled all 10 documents with the v0.2.0 codebase.

## Document Index

| # | Document | Purpose |
|---|---|---|
| 01 | [SRS — Software Requirements](01-SRS-Software-Requirements.md) | Software Requirements Specification (functional + non-functional). |
| 02 | [SDD — Software Design](02-SDD-Software-Design.md) | Software Design Description: architecture, module table, schema, RBAC, crypto, background-task lifecycle. |
| 03 | [Quality Model — ISO 25010](03-Quality-Model-ISO-25010.md) | ISO 25010 quality characteristics + conformance levels. |
| 04 | [Security Control Matrix — ISO 27001](04-Security-Control-Matrix-ISO-27001.md) | ISO 27001 control conformance + audit closure summary (M-01..M-08). |
| 05 | [Risk Register — ISO 31000](05-Risk-Register-ISO-31000.md) | ISO 31000 risk register (R-001..R-040). |
| 06 | [SDLC — ISO 12207](06-SDLC-ISO-12207.md) | ISO 12207 life cycle process + tooling versions. |
| 07 | [Licensing Architecture](07-Licensing-Architecture.md) | License model, hardware fingerprint, Ed25519 signing, verification sequence, revocation, grace period, transfer, keygen/ project. |
| 08 | [Deployment & Installation Guide](08-Deployment-Installation-Guide.md) | Install / upgrade / troubleshoot; NSIS hooks; ProgramData layout; pairing; TLS pinning; first-run admin; license install; backup/restore; multi-PC topology; security summary. |
| 09 | [UI/UX Design Specification](09-UI-UX-Design-Specification.md) | Design system (palette, typography, tokens, primitives) + 20 page/feature specs + WCAG 2.2 AA checklist. |
| 10 | [Licensing Workflow Guide](10-Licensing-Workflow-Guide.md) | Operational license issuance runbook: dev auto-licensing, production 6-step flow, keygen/ binaries, key rotation, security checklist. |
| — | [CHANGELOG](CHANGELOG.md) | Version history (v0.1.0 → v0.2.0). Start here for "what changed". |
| — | [../DESIGN_SYSTEM.md](../DESIGN_SYSTEM.md) | ⚠️ **SUPERSEDED by 09-UI-UX-Design-Specification.md** — retained for history; do not edit. |

## Version

Current: **v0.2.0** (2025-07-08) — post Phase 2 Batches 0-3 + Batch 4 documentation reconciliation.

Each document carries a version banner at the top:
> **Document version: v0.2.0 — updated 2025-07-08 after Phase 2 Batches 0-3 implementation. See CHANGELOG.md for details.**

## Document map (which doc to read first)

| If you want to know... | Read |
|---|---|
| What the system does | 01-SRS §3 (Functional Requirements) + 09-UI/UX (page specs) |
| How the system is built | 02-SDD §3 (module table) + §4 (schema) |
| Is it secure? | 04-Security Matrix (M-01..M-08 closure) + 05-Risk Register (R-001..R-040) |
| How do I install it? | 08-Deployment Guide |
| How do I issue a license? | 10-Licensing Workflow Guide + 07-Licensing Architecture §5.6 (keygen/) |
| Is it accessible? | 09-UI/UX §14 (WCAG 2.2 AA checklist) |
| What changed since v0.1.0? | CHANGELOG.md (this folder) |
| What's still Planned? | 06-SDLC §11 (process improvement) + 03-Quality Model §11 (gap remediation) |

## Documentation Improvement Policy

Per the RCTF prompt, documentation is collaborative. If you find an inconsistency between a doc and the code:

1. **Propose** an update in your worklog entry (do not silently edit).
2. **Await approval** from the Documentation Team lead before editing.
3. **Edit** with the v0.2.0 convention markers:
   - `**[Implemented v0.2.0 (Batch X CR-YY)]**` — code now matches a previously-aspirational doc claim.
   - `**[Improved v0.2.0]**` — code now partially matches; gap remains.
   - `**[Planned Phase 2]**` / `**[Planned Batch 5]**` — doc claim not yet met; future work.
   - `~~strikethrough~~` on stale v0.1.0 claims that are no longer accurate, with a `**[Resolved v0.2.0]**` note.
4. **Append a revision-history row** to the doc's "Revision history" subsection.
5. **Update the CHANGELOG** if the change is user-visible.

## Cross-reference quick links

- **Project root:** `/home/z/my-project/hospital-mgt-extracted/hospital-mgt`
- **RCTF prompt:** `/home/z/my-project/upload/VitalFlow_HMS_RCTF_Antigravity_Enterprise_Review_Prompt.md`
- **Phase 1 audit report:** `PHASE1_AUDIT_REPORT.md` (project root)
- **Implementation worklog:** `/home/z/my-project/worklog.md` (full per-batch detail; per-batch entries are tagged `Task ID: B0` / `B1` / `B2` / `B3-A` / `B3-B` / `B3-C` / `B4-A` / `B4-B` / `B4-C`)
- **Keygen project README:** `keygen/README.md`
- **PostgreSQL setup (engineering):** `src-tauri/SETUP_POSTGRES_BINARIES.md`
- **Tauri deployment guide:** `src-tauri/HMS_DEPLOYMENT_GUIDE.md` (legacy; superseded by 08-Deployment-Installation-Guide.md)

---

_End of `docs/README.md`. For the version history see `CHANGELOG.md`._
