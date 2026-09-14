import * as React from "react";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";

/**
 * Dialog — enterprise modal. Uses the design system's large
 * radius (--radius-lg) and elevated shadow. Title uses the
 * display-sm class for editorial weight. Animations are smooth
 * and respectful of reduced-motion.
 *
 * UX-2026-09-13 (dialog responsiveness): the content is centered on BOTH
 * axes (left-50% top-50% + translate) and sized dynamically against the
 * viewport so it behaves on every screen — clinic desktop, Windows
 * display scaling (100/125/150%), and narrow mobile widths:
 *   - width:  calc(100vw - 2rem)  → always a 1rem side margin; call-site
 *     max-w-* still caps it on wider screens (twMerge keeps both).
 *   - height: max-h calc(100dvh - 2rem) with an INTERNAL scroll wrapper →
 *     a tall form (e.g. the 20-field patient registration) scrolls inside
 *     the centered box instead of being clipped above the fold, and the
 *     Close (X) stays pinned to the visible top-right corner.
 *   - padding/rounding tighten on small screens (p-4, radius-md) and
 *     relax to the desktop rhythm at the sm: breakpoint.
 */
const Dialog = DialogPrimitive.Root;
const DialogTrigger = DialogPrimitive.Trigger;
const DialogPortal = DialogPrimitive.Portal;
const DialogClose = DialogPrimitive.Close;

const DialogOverlay = React.forwardRef<
  React.ComponentRef<typeof DialogPrimitive.Overlay>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Overlay>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Overlay
    ref={ref}
    className={cn(
      "fixed inset-0 z-50 bg-black/60 backdrop-blur-[2px] data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
      className,
    )}
    {...props}
  />
));
DialogOverlay.displayName = DialogPrimitive.Overlay.displayName;

const DialogContent = React.forwardRef<
  React.ComponentRef<typeof DialogPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Content>
>(({ className, children, ...props }, ref) => (
  <DialogPortal>
    <DialogOverlay />
    <DialogPrimitive.Content
      ref={ref}
      className={cn(
        "fixed left-1/2 top-1/2 z-50 flex max-h-[calc(100dvh-2rem)] w-[calc(100vw-2rem)] max-w-lg -translate-x-1/2 -translate-y-1/2 flex-col rounded-[var(--radius-md)] border border-border bg-background p-4 shadow-xl duration-200 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 sm:rounded-[var(--radius-lg)] sm:p-6",
        className,
      )}
      {...props}
    >
      {/* Scroll area: children keep the original grid gap-4 rhythm, but
          overflow inside the centered box. The flex-1/min-h-0 pair makes
          the wrapper shrink-fit the dialog's max-height instead of the
          dialog overflowing the viewport. */}
      <div className="grid min-h-0 flex-1 gap-4 overflow-y-auto">
        {children}
      </div>
      {/* Close is anchored to the non-scrolling content box, so it stays
          visible at the top-right even while long forms scroll. */}
      <DialogPrimitive.Close className="absolute right-4 top-4 rounded-[var(--radius-sm)] bg-card/80 p-1 text-muted-foreground opacity-70 ring-offset-background transition-all hover:bg-muted hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring/40 disabled:pointer-events-none">
        <X className="h-4 w-4" />
        <span className="sr-only">Close</span>
      </DialogPrimitive.Close>
    </DialogPrimitive.Content>
  </DialogPortal>
));
DialogContent.displayName = DialogPrimitive.Content.displayName;

const DialogHeader = ({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) => (
  <div
    className={cn(
      "flex flex-col space-y-1.5 text-center sm:text-left",
      className,
    )}
    {...props}
  />
);
DialogHeader.displayName = "DialogHeader";

const DialogFooter = ({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) => (
  <div
    className={cn(
      "flex flex-col-reverse gap-2 sm:flex-row sm:justify-end sm:gap-2",
      className,
    )}
    {...props}
  />
);
DialogFooter.displayName = "DialogFooter";

const DialogTitle = React.forwardRef<
  React.ComponentRef<typeof DialogPrimitive.Title>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Title>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Title
    ref={ref}
    className={cn(
      "text-display-md text-foreground leading-tight tracking-tight",
      className,
    )}
    {...props}
  />
));
DialogTitle.displayName = DialogPrimitive.Title.displayName;

const DialogDescription = React.forwardRef<
  React.ComponentRef<typeof DialogPrimitive.Description>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Description>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Description
    ref={ref}
    className={cn("text-sm text-muted-foreground leading-relaxed", className)}
    {...props}
  />
));
DialogDescription.displayName = DialogPrimitive.Description.displayName;

export {
  Dialog,
  DialogPortal,
  DialogOverlay,
  DialogClose,
  DialogTrigger,
  DialogContent,
  DialogHeader,
  DialogFooter,
  DialogTitle,
  DialogDescription,
};
