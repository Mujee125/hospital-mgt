# SEC-FIX-2026-09-27 — Clinical-safety remediation batch

Remediation of four findings raised in the engineering review. Each section
states the defect, the change, and — importantly — **what was deliberately not
changed and why**, because a security fix that silently alters clinical
behaviour is its own incident.

---

## FIX-A — the ABO-incompatible emergency-release bypass

### The defect

`issue_blood` refused an ABO/Rh-incompatible release **unless** the caller set
`issue_type = 'emergency' | 'uncrossmatched'` and put at least one non-space
character in `clinical_indication`:

```rust
if !is_emergency_override { return Err(...); }
if !has_indication { return Err(...); }
// fall through and claim the unit
```

So the gate against an acute haemolytic reaction (fatal) was one free-text
word. `"urgent"` released O-positive blood to an A-positive patient. The audit
row written afterwards did not record that an override had happened at all, so
the event was invisible to review.

### The change — HYBRID QUARANTINE

The override survives. Clinically it must: an untyped trauma patient gets
O-negative before the lab finishes typing, and a system that blocks that kills
people. What changes is that the override becomes a **structured, attributable,
reviewed act**:

| Control | Where | Why |
|---|---|---|
| Closed `override_reason_code` (5 values, mirrored by a SQL `CHECK`) | `validate_emergency_override` | A free-text reason is not a reason. A closed vocabulary can be counted, trended and escalated. |
| `clinical_indication` ≥ **25** characters | `validate_emergency_override` | The old check accepted one character. 25 is the floor at which the text has to describe a clinical situation. |
| `physician_order_ref` (≤ 80 chars) | `validate_emergency_override` | An emergency release is an act someone ordered. The reference is the pointer to that order. |
| Per-user rolling **24 h cap** (default 3) | `enforce_emergency_release_quota` | One person issuing incompatible blood all night is either a mass casualty (a supervisor should know) or a compromised account. Unverified-and-overdue releases count **double**, so a stalled review loop bites immediately. |
| **Second-person co-signature** with a deadline | `verify_blood_issue` | A control nobody can close does not exist. |
| Typing snapshot + reason + order in the unit history and the audit row | `issue_blood` | The override must be visible without joining tables. |

**The blood is never blocked.** The unit keeps `status = 'issued'`, transfusion
is still allowed, and the release returns `Ok(issue_id)`. What is quarantined is

### Second-person verification

`verify_blood_issue` requires **all** of:

1. `bloodbank.verify` — a permission the **lab technician** role now holds and
   the **doctor** role deliberately does not. Without the lab-tech grant a
   single-tech deployment could only close the queue as an admin, and in
   practice nobody closes it.
2. The reviewer's own password (Argon2id, via `auth::verify_password_async`).
   An open session is not enough.
3. `agreed = true` — an affirmative act, not a default.
4. ≥ 10 characters of "what did you check". `"verified"` is not an audit trail.
5. `issued_by_user_id != session.user_id` — self-verification is refused, in SQL
   and in the UI (the issuer's own rows are filtered out of the queue).
6. A `FOR UPDATE` lock on the oversight row, so two verifiers clicking at once
   cannot both write a signature.

Disagreement is a first-class outcome (`discrepancy`), audited and routed to
the blood-bank role as a notification. A control with only an approve button
pushes people towards approving.

### Deliberate non-changes

- The release is **not** retroactively invalidated. The unit may already be
  transfused; marking that invalid would be a lie about what happened.
- The notification is **best-effort and post-commit**. A failed notification
  must not turn a completed release into a client-visible failure. The pending
  queue is the authoritative safety net.
- The quota is **per user, not per patient or per unit** — a fixed global cap
  would break mass-casualty care, which is the one case the escape hatch exists
  for. `blood.emergency_release_24h_limit` is retunable at runtime.

---

## FIX-B — unbounded patient / clinical list queries

`get_patients`, `get_appointments`, `get_bills` and `get_encounters` had **no
`LIMIT`**. Mounting a page serialized the whole registry — including the
`TEXT[]` allergies and chronic_conditions on every row — across the IPC
boundary. `get_bills` was worse: two correlated `SUM` subqueries per row, on
the money table.

- All four now take `(limit, offset)` and go through `db::page_bounds`, which
  clamps in **one** place: default 250, hard max 1000, negative offsets
  normalized. A caller cannot reintroduce an unbounded fetch by omission.
- `ORDER BY` gained an `id` tiebreaker. Paging on `created_at` alone is
  unstable: two rows created in the same millisecond can swap pages between
  calls and one of them silently never appears.
- New `search_patient_options` returns a deliberately tiny projection
  (`PatientOption`: name, MRN, phone, DOB, gender, blood group) for pickers. A
  dropdown must not ship — or render — clinical detail its list did not intend
  to show, and it returns **nothing** for an empty search term rather than an
  arbitrary alphabetical slice of the hospital's patients.
- The Patients page now says when it is sitting on the server cap, and pushes a
  ≥ 2-character search term to the server. A silent subset reads as "these are
  all our patients", and a receptionist concluding a patient is unregistered
  because the registry is longer than one page is a real harm.
- Matching indexes: `idx_patients_created_id`, `idx_encounters_visit_id`,
  `idx_bills_created_id`, `idx_appointments_date_time`.


---

## FIX-C — inventory stock integrity

- **Negative stock is now impossible in the database.** A
  `chk_inventory_stock_non_negative` CHECK constraint is attached (after
  flooring pre-existing negative rows to 0 with a loud log line, so one legacy
  row cannot abort startup on a Sunday boot). Every writer computed a balance in
  Rust and wrote an ABSOLUTE value — correct only as long as every future
  writer remembered to lock first.
- `adjust_inventory` and pharmacy dispensing now use **relative, guarded**
  updates: `SET stock_quantity = stock_quantity ± $1 ... WHERE stock_quantity ±
  $1 >= 0`. The insufficient-stock decision moves from "Rust computed it" to
  "the database refused it", and dispensing uses `RETURNING` so the movement
  ledger records the balance that was actually written.
- `create_inventory_item` is now **transactional**. The item row and its
  opening-balance movement were two separate writes, the second a silent
  `let _ =` — a failure there left stock that existed nowhere in the ledger the
  module's own invariant depends on.

---

## FIX-D — pharmacy stock matching

The dispenser resolved stock with `ORDER BY id ASC LIMIT 1` — "whichever row
the planner created first". For expiry-dated stock that is the opposite of
FEFO, and because the catalog path matches the item name against **both**
`brand_name` and `generic_name`, it could pick a brand line when a generic line
was equally valid, with no signal that a choice had been made on the
pharmacist's behalf.

The replacement: never match expired stock, order the survivors by soonest
expiry (FEFO, undated last), and **refuse with both candidates named** when the
two best matches are different products. Same-name lines differing only by batch
are not ambiguous — FEFO resolves them, which is the point of having an expiry
date. The candidate list is capped at 50 so pathological data cannot become its
own denial of service.

---

## Also fixed

- `db::setting_i64` decoded as `Option<String>` and called `.flatten()` twice.
  It now decodes as `String`, so there is a single `Option` and no guess about
  how many flattens are correct. These keys gate a **safety** limit, so a
  silently-defaulted one is a silent policy change.
- The list commands previously relied on a client that sent the right argument
  count; the new paging parameters are `Option` and defaulted server-side.

---

## Tests

`cargo test --lib blood_bank` (95 passing) adds ten cases:

- `fix_a_override_rejects_a_one_word_indication` — **the regression test** for
  the original defect.
- `fix_a_override_requires_a_reason_code` / `rejects_a_free_text_reason` /
  `requires_a_physician_order_reference` / `rejects_an_over_long_order_reference`
  / `accepts_a_full_justification` / `trims_before_validating`.
- `fix_a_override_reason_codes_match_the_database_constraint` — the tripwire
  that keeps the Rust list, the SQL `CHECK` and the TS constant from drifting.
  Drift would make every override fail at 3 AM with an opaque constraint error.
- `fix_a_indication_floor_is_25_characters` — the same number is quoted in the
  Rust constant, the error text and the UI counter.
- `fix_b_page_bounds_defaults_and_clamps` — the clamp edges.

`src/lib/bloodbank.test.ts` (42 passing) adds the two new hook exports, the
typeahead hook, the reason-vocabulary drift tripwire, and the
`bloodbank.verify` permission constant.

The DB-backed paths (quota counting, self-verification refusal, concurrent
co-signing) need a live Postgres and a Tauri runtime; they are not covered by
unit tests here and are the first thing to write if this is picked up again.

A `pg_trgm` GIN index for the `%term%` search predicate is **not** attempted:
`CREATE EXTENSION` requires superuser, which locked-down hospital installs do
not grant. The scan still happens; what changed is that the result is capped
and the payload is small.

the *paperwork* — the issue row carries
`override_verification_required = TRUE` until a **different** user, holding
`bloodbank.verify`, re-enters their own password and records what they checked.

A hard tier (block the release until a second signature exists) is a
one-branch change in `issue_blood` and was **rejected**: 3 AM staffing reality
was judged by the blood-bank SOP to make it unsafe. That decision is recorded
here so the next reviewer does not "fix" it without the same conversation.
