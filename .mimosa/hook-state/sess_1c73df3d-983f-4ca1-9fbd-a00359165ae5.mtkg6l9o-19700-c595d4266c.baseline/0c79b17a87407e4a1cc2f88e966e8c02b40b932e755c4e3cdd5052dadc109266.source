import * as React from "react";
import { cn } from "@/lib/utils";

/**
 * Textarea — multi-line form field. Mirrors Input styling so forms
 * feel cohesive. Min height keeps a comfortable reading area.
 */
export type TextareaProps = React.TextareaHTMLAttributes<HTMLTextAreaElement>;

const Textarea = React.forwardRef<HTMLTextAreaElement, TextareaProps>(
  ({ className, ...props }, ref) => {
    return (
      <textarea
        className={cn(
          "flex min-h-[88px] w-full rounded-[var(--radius)] border border-border bg-muted/40 px-3.5 py-2.5 text-sm ring-offset-background transition-all duration-200 placeholder:text-muted-foreground/60 focus-visible:outline-none focus-visible:bg-card focus-visible:border-primary/50 focus-visible:ring-2 focus-visible:ring-primary/15 disabled:cursor-not-allowed disabled:opacity-50 resize-y",
          className,
        )}
        ref={ref}
        {...props}
      />
    );
  },
);
Textarea.displayName = "Textarea";

export { Textarea };
