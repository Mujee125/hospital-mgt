import React, { useState } from "react";
import { cn } from "@/lib/utils";

/**
 * InputField — floating-label input. A legacy helper kept for
 * compatibility; modernized to use design-system tokens. Prefer
 * the Label + Input + Field (shared.tsx) pattern for new forms.
 */
export const InputField: React.FC<{
  label: string;
  name: string;
  type?: string;
  value: string | number;
  onChange: (
    e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>,
  ) => void;
  onKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  required?: boolean;
  className?: string;
}> = ({ label, name, type = "text", value, onChange, onKeyDown, required, className }) => {
  const [isFocused, setIsFocused] = useState(false);
  const shouldFloatLabel = isFocused || value !== "";
  return (
    <div className="relative flex items-center mb-4">
      <input
        type={type}
        name={name}
        id={name}
        value={value}
        onChange={onChange}
        onFocus={() => setIsFocused(true)}
        onBlur={() => setIsFocused(false)}
        required={required}
        onKeyDown={onKeyDown}
        className={cn(
          "peer w-full rounded-[var(--radius)] border bg-muted/40 px-3.5 py-2.5 min-h-[44px] text-sm transition-all duration-200 focus:outline-none focus:bg-card focus:ring-2 focus:ring-primary/15 hover:border-primary/30",
          value ? "border-primary/50" : "border-border",
          className,
        )}
      />
      <label
        htmlFor={name}
        className={cn(
          "absolute left-3 transition-all duration-200 text-sm pointer-events-none bg-card px-1",
          shouldFloatLabel
            ? "-top-2 text-xs font-semibold text-primary"
            : "top-1/2 -translate-y-1/2 text-muted-foreground",
        )}
      >
        {label}
      </label>
    </div>
  );
};
