import * as React from "react";
import { cn } from "@/lib/utils";

type SwitchProps = Omit<
  React.ButtonHTMLAttributes<HTMLButtonElement>,
  "onChange"
> & {
  checked: boolean;
  onCheckedChange?: (checked: boolean) => void;
};

export const Switch = React.forwardRef<HTMLButtonElement, SwitchProps>(
  (
    { checked, onCheckedChange, className, disabled, onClick, ...props },
    ref,
  ) => (
    <button
      ref={ref}
      type="button"
      role="switch"
      aria-checked={checked}
      data-state={checked ? "checked" : "unchecked"}
      disabled={disabled}
      className={cn(
        "relative inline-flex h-6 w-11 items-center rounded-full border transition-all",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60 focus-visible:ring-offset-2 focus-visible:ring-offset-background",
        checked
          ? "bg-primary border-primary/70 shadow-[0_0_0_1px_rgba(255,255,255,0.03),0_0_18px_rgba(94,106,210,0.26)]"
          : "bg-secondary/80 border-border/70",
        disabled && "cursor-not-allowed opacity-50",
        className,
      )}
      onClick={(event) => {
        // We intentionally rely on the native `disabled` behavior to block
        // clicks, keeping the handler branch-free and predictable.
        onCheckedChange?.(!checked);
        onClick?.(event);
      }}
      {...props}
    >
      <span
        className={cn(
          "inline-block h-5 w-5 rounded-full bg-background shadow-[0_1px_2px_rgba(0,0,0,0.3)] transition-transform",
          checked ? "translate-x-5" : "translate-x-0.5",
        )}
      />
    </button>
  ),
);

Switch.displayName = "Switch";
