/* eslint-disable react-refresh/only-export-components */
import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

/**
 * Badge — compact label pill. Semantic variants map to the
 * design-system status tokens so the lifecycle colours are
 * identical everywhere (tables, cards, headers). Uses a soft
 * tinted background with a solid text colour for AA contrast.
 */
const badgeVariants = cva(
  "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ring/40 whitespace-nowrap",
  {
    variants: {
      variant: {
        default:
          "border-transparent bg-primary text-primary-foreground",
        secondary:
          "border-transparent bg-secondary text-secondary-foreground",
        destructive:
          "border-transparent bg-destructive text-destructive-foreground",
        outline: "border-border text-foreground",
        scheduled:
          "border-transparent bg-status-scheduled/12 text-status-scheduled",
        confirmed:
          "border-transparent bg-status-confirmed/12 text-status-confirmed",
        completed:
          "border-transparent bg-status-completed/12 text-status-completed",
        cancelled:
          "border-transparent bg-status-cancelled/12 text-status-cancelled",
        no_show:
          "border-transparent bg-status-no-show/12 text-status-no-show",
        success:
          "border-transparent bg-success/12 text-success",
        warning:
          "border-transparent bg-warning/12 text-warning",
        info: "border-transparent bg-info/12 text-info",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return (
    <div className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}

export { Badge, badgeVariants };
