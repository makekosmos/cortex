import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";

import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-[510] transition-[background-color,border-color,color,box-shadow,transform] duration-150 disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 shrink-0 [&_svg]:shrink-0 outline-none focus-visible:ring-2 focus-visible:ring-ring/60 focus-visible:ring-offset-2 focus-visible:ring-offset-background aria-invalid:ring-destructive/20 aria-invalid:border-destructive",
  {
    variants: {
      variant: {
        default:
          "bg-primary text-primary-foreground shadow-[0_1px_0_rgba(255,255,255,0.04),0_10px_30px_rgba(94,106,210,0.24)] hover:bg-[#7170ff] hover:shadow-[0_1px_0_rgba(255,255,255,0.04),0_12px_34px_rgba(94,106,210,0.32)]",
        destructive:
          "bg-destructive text-white shadow-[0_1px_0_rgba(255,255,255,0.04),0_10px_30px_rgba(220,38,38,0.18)] hover:bg-destructive/90 focus-visible:ring-destructive/30",
        outline:
          "border border-border/80 bg-card/70 text-foreground shadow-[0_1px_0_rgba(255,255,255,0.03)] hover:bg-card hover:border-border/90",
        secondary:
          "border border-border/70 bg-secondary/80 text-secondary-foreground shadow-[0_1px_0_rgba(255,255,255,0.03)] hover:bg-secondary",
        ghost:
          "text-secondary-foreground hover:bg-accent/70 hover:text-foreground",
        link: "text-primary underline-offset-4 hover:text-[#828fff] hover:underline",
      },
      size: {
        default: "h-9 px-4 py-2 has-[>svg]:px-3",
        sm: "h-8 rounded-md gap-1.5 px-3 has-[>svg]:px-2.5",
        lg: "h-10 rounded-md px-5 has-[>svg]:px-4",
        icon: "size-9 rounded-md",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
);

export const Button = React.forwardRef<
  HTMLButtonElement,
  React.ComponentProps<"button"> &
    VariantProps<typeof buttonVariants> & {
      asChild?: boolean;
    }
>(({ className, variant, size, asChild = false, type, ...props }, ref) => {
  const Comp = asChild ? Slot : "button";

  return (
    <Comp
      ref={ref}
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...(asChild ? {} : { type: type ?? "button" })}
      {...props}
    />
  );
});

Button.displayName = "Button";

export { buttonVariants };
