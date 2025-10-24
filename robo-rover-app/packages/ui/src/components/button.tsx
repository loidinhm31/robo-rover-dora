import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@repo/ui/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-2xl text-sm font-bold transition-all duration-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400 focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "bg-gradient-to-r from-cyan-500 to-blue-500 text-white shadow-lg hover:shadow-xl hover:scale-105",
        destructive:
          "bg-gradient-to-r from-red-600 to-red-500 text-white shadow-lg hover:shadow-xl hover:scale-105",
        outline:
          "border-2 border-white/30 bg-white/10 backdrop-blur-md text-white shadow-sm hover:bg-white/20 hover:border-white/40",
        secondary:
          "bg-gradient-to-r from-purple-500 to-pink-500 text-white shadow-lg hover:shadow-xl hover:scale-105",
        ghost: "text-white hover:bg-white/10 hover:text-white",
        link: "text-cyan-300 underline-offset-4 hover:underline",
        gradient:
          "bg-gradient-to-r from-orange-400 via-orange-500 to-yellow-500 text-white shadow-lg hover:shadow-xl hover:scale-105",
        glass:
          "backdrop-blur-xl bg-white/10 border border-white/20 text-white shadow-lg hover:bg-white/20",
      },
      size: {
        default: "h-10 px-6 py-2",
        sm: "h-8 rounded-xl px-4 text-xs",
        lg: "h-12 rounded-2xl px-10 text-base",
        icon: "h-10 w-10",
        xl: "h-14 rounded-3xl px-12 text-lg",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";

export { Button, buttonVariants };