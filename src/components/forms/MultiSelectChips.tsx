/**
 * MultiSelectChips — a selectable-options control for clinical list
 * fields (allergies, chronic conditions, qualifications).
 *
 * UX-2026-09-13 follow-up: registration staff needed to SELECT one or
 * more from known options instead of free-typing every record; an
 * explicit "+ Add" button alongside the options lets them add anything
 * the preset list doesn't cover.
 *
 * Storage contract (unchanged): the value is the same comma-separated
 * free-text string the previous Textareas produced — presets are an
 * input affordance, not a schema change. Parsing is case-insensitive
 * de-duplicating so a hand-typed "penicillin" and the chip "Penicillin"
 * collapse into one selection (lib/clinicalPresets parseSelection).
 *
 * Accessibility: role="group" with aria-label; every option chip is a
 * real <button> carrying aria-pressed; the Add input is labelled via
 * the group (id prop targets the input so FormField's htmlFor resolves).
 */
import { useState } from "react";
import { Plus, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { parseSelection, joinSelection } from "@/lib/clinicalPresets";

export function MultiSelectChips({
  id,
  label,
  options,
  value,
  onChange,
  placeholder,
  disabled = false,
  className = "",
}: {
  /** Applied to the Add input — the parent FormField's htmlFor label. */
  id: string;
  /** Announced as the group's accessible name. */
  label: string;
  /** Selectable options (preset ∪ DB-loaded ∪ session-added). */
  options: readonly string[];
  /** Stored comma-separated value — unchanged backend contract. */
  value: string;
  onChange: (next: string) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
}) {
  const [draft, setDraft] = useState("");
  const items = parseSelection(value);

  const lower = (s: string) => s.trim().toLowerCase();
  const has = (item: string) => items.some((i) => lower(i) === lower(item));

  /** Toggle a preset/known option chip on or off. */
  const toggle = (item: string) => {
    const next = has(item)
      ? items.filter((i) => lower(i) !== lower(item))
      : [...items, item];
    onChange(joinSelection(next));
  };

  /** Add the typed text as new selection(s) — the + Add button, Enter,
   *  or comma. Parsed with parseSelection so a trailing/pasted comma
   *  ("Kiwi," or "Penicillin, Latex") adds clean items, never one item
   *  containing commas (which would corrupt the stored value format).
   *  A no-op (only duplicates) clears the draft without firing onChange. */
  const addDraft = () => {
    const additions = parseSelection(draft);
    if (additions.length === 0) {
      setDraft("");
      return;
    }
    const next = [...items];
    let added = false;
    for (const addition of additions) {
      if (!next.some((i) => lower(i) === lower(addition))) {
        next.push(addition);
        added = true;
      }
    }
    if (added) onChange(joinSelection(next));
    setDraft("");
  };

  const remove = (item: string) => {
    onChange(joinSelection(items.filter((i) => lower(i) !== lower(item))));
  };

  /** Options never selected still render (available to select); items
   *  not in the option list are "custom" chips with a remove control. */
  const customItems = items.filter(
    (i) => !options.some((o) => lower(o) === lower(i)),
  );

  return (
    <div
      role="group"
      aria-label={label}
      className={cn(
        "flex min-h-10 w-full flex-wrap items-center gap-1.5 rounded-[var(--radius)] border border-border bg-card px-2 py-1.5 focus-within:border-primary/50 focus-within:ring-2 focus-within:ring-primary/15",
        disabled && "opacity-60 pointer-events-none",
        className,
      )}
    >
      {options.map((opt) => (
        <button
          key={opt}
          type="button"
          aria-pressed={has(opt)}
          disabled={disabled}
          onClick={() => toggle(opt)}
          className={cn(
            "rounded-full border px-2.5 py-1 text-xs font-medium transition-colors",
            has(opt)
              ? "border-primary bg-primary text-primary-foreground"
              : "border-border bg-background text-muted-foreground hover:border-primary/40 hover:text-foreground",
          )}
        >
          {opt}
        </button>
      ))}

      {customItems.map((item) => (
        <span
          key={item}
          className="inline-flex items-center gap-1 rounded-full border border-primary/50 bg-primary/10 px-2.5 py-1 text-xs font-medium text-foreground"
        >
          {item}
          <button
            type="button"
            aria-label={`Remove ${item}`}
            disabled={disabled}
            onClick={() => remove(item)}
            className="text-muted-foreground hover:text-destructive focus:outline-none focus:ring-2 focus:ring-ring/40 rounded-full"
          >
            <X className="h-3 w-3" aria-hidden="true" />
          </button>
        </span>
      ))}

      <span className="inline-flex flex-1 min-w-[10rem] items-center gap-1.5">
        <input
          id={id}
          type="text"
          value={draft}
          disabled={disabled}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === ",") {
              // Never submit the surrounding form from this input.
              e.preventDefault();
              addDraft();
            }
          }}
          placeholder={placeholder ?? "Add other…"}
          aria-label={`Add new ${label.toLowerCase()} option`}
          className="h-7 w-full min-w-0 flex-1 bg-transparent text-xs outline-none placeholder:text-muted-foreground/70"
        />
        <button
          type="button"
          onClick={addDraft}
          disabled={disabled || draft.trim() === ""}
          className="inline-flex h-6 shrink-0 items-center gap-1 rounded-[var(--radius-sm)] border border-border px-2 text-[11px] font-semibold text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground disabled:opacity-40"
        >
          <Plus className="h-3 w-3" aria-hidden="true" /> Add
        </button>
      </span>
    </div>
  );
}
