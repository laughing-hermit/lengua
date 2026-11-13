import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

const VARIANTS = {
  default: "bg-primary text-primary-foreground shadow-xs hover:bg-primary/90",
  destructive: "bg-destructive text-white shadow-xs hover:bg-destructive/90",
  outline: "border bg-background shadow-xs hover:bg-accent hover:text-accent-foreground",
  secondary: "bg-secondary text-secondary-foreground shadow-xs hover:bg-secondary/80",
  ghost: "hover:bg-accent hover:text-accent-foreground",
  link: "text-primary underline-offset-4 hover:underline",
} as const;

const SIZES = {
  default: "h-9 px-4 py-2",
  sm: "h-8 gap-1.5 rounded-md px-3",
  lg: "h-10 rounded-md px-6",
  icon: "size-9",
} as const;

export function Button({
  className,
  variant = "default",
  size = "default",
  type = "button",
  ...props
}: ComponentProps<"button"> & {
  readonly variant?: keyof typeof VARIANTS;
  readonly size?: keyof typeof SIZES;
}) {
  return (
    <button
      data-slot="button"
      type={type}
      className={cn(
        "inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium outline-none transition-all focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50",
        VARIANTS[variant],
        SIZES[size],
        className,
      )}
      {...props}
    />
  );
}
