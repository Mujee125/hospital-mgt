/**
 * Curated clinical preset lists (UX-2026-09-13 follow-up: selectable
 * options for patient allergies / chronic conditions and practitioner
 * specialization / qualifications).
 *
 * These presets are INPUT AFFORDANCES only — the stored values remain
 * free-text strings (the unchanged backend contract: patients.allergies,
 * patients.chronic_conditions, doctors.specialization,
 * doctors.qualification are all Option/String). The MultiSelectChips
 * control joins selections into the same comma-separated format the
 * free-text fields always produced, so existing records parse cleanly
 * and the PatientSafetyBanner / pickers keep working unchanged.
 *
 * Preset names deliberately contain NO commas (comma is the value-list
 * separator). Em dashes separate subtypes instead.
 */

// ── Drug / food / environmental allergies (SRS §2.2: "known allergies
//    (drug/food/environmental)"). No "no known allergies" chip: writing
//    NKDA text would make the safety banner render a red alert for it;
//    an empty selection already means "none recorded".
export const ALLERGY_PRESETS: readonly string[] = [
  "Penicillin",
  "Cephalosporins",
  "Sulfa drugs",
  "Aspirin / NSAIDs",
  "Codeine / Opioids",
  "Insulin",
  "Iodine / contrast dye",
  "Latex",
  "Peanut",
  "Tree nuts",
  "Shellfish",
  "Fish",
  "Eggs",
  "Milk",
  "Soy",
  "Wheat / Gluten",
  "Bee stings",
  "Pollen",
  "Dust mites",
];

// ── Long-term conditions common in the OPD/IPD population.
export const CHRONIC_CONDITION_PRESETS: readonly string[] = [
  "Diabetes — Type 1",
  "Diabetes — Type 2",
  "Hypertension",
  "Ischemic heart disease",
  "Heart failure",
  "Asthma",
  "COPD",
  "Chronic kidney disease",
  "Hepatitis B",
  "Hepatitis C",
  "Liver cirrhosis",
  "Hypothyroidism",
  "Hyperthyroidism",
  "Epilepsy",
  "Migraine",
  "Osteoarthritis",
  "Rheumatoid arthritis",
  "Thalassemia",
  "Tuberculosis",
  "GERD",
  "Chronic anemia",
  "Depression",
];

// ── Practitioner specialties (union with the DB-distinct list at call
//    sites — the DB list captures anything already registered).
export const SPECIALIZATION_PRESETS: readonly string[] = [
  "General Medicine",
  "Family Medicine",
  "Cardiology",
  "Dermatology",
  "ENT",
  "Endocrinology",
  "Gastroenterology",
  "Gynecology & Obstetrics",
  "Nephrology",
  "Neurology",
  "Oncology",
  "Ophthalmology",
  "Orthopedics",
  "Pediatrics",
  "Physiotherapy",
  "Psychiatry",
  "Pulmonology",
  "Urology",
];

// ── Post-nominal qualifications (multi-select: joined "MBBS, MD, FCPS").
export const QUALIFICATION_PRESETS: readonly string[] = [
  "MBBS",
  "MD",
  "MS",
  "FCPS",
  "MRCP",
  "FRCS",
  "FRCP",
  "MRCOG",
  "DCH",
  "DOMS",
  "DA",
  "RMP",
];

// ── Value parsing / joining ─────────────────────────────────────────────────

/**
 * Split a stored comma-separated value into display items: trims, drops
 * empties, and de-duplicates case-insensitively (keeping the first
 * occurrence's casing) so a hand-typed "penicillin" and the preset chip
 * "Penicillin" collapse into one selection instead of two.
 */
export function parseSelection(value: string | null | undefined): string[] {
  if (!value) return [];
  const seen = new Set<string>();
  const items: string[] = [];
  for (const raw of value.split(",")) {
    const item = raw.trim();
    if (item === "") continue;
    const key = item.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    items.push(item);
  }
  return items;
}

/** Join display items back into the stored comma-separated value. */
export function joinSelection(items: readonly string[]): string {
  return items.join(", ");
}

/**
 * Case-insensitive option merge (presets first, then DB-loaded values),
 * de-duplicated — used to build chip/select option lists.
 */
export function mergeOptions(
  presets: readonly string[],
  ...additional: (string[] | undefined)[]
): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const opt of [...presets, ...additional.flat()]) {
    const trimmed = (opt ?? "").trim();
    if (trimmed === "") continue;
    const key = trimmed.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(trimmed);
  }
  return out;
}
