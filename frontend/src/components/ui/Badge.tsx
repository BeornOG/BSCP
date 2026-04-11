import type { FC, ReactNode } from "react";;

interface BadgeProps {
  variant?: 'default' | 'success' | 'warning' | 'danger';
  children: ReactNode;
}

const variantStyles = {
  default: 'bg-[#232529] text-[#71747a]',
  success: 'bg-green-500/15 text-green-400',
  warning: 'bg-amber-500/15 text-amber-400',
  danger: 'bg-red-500/15 text-red-400',
};

const Badge: FC<BadgeProps> = ({ variant = 'default', children }) => (
  <span
    className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${variantStyles[variant]}`}
  >
    {children}
  </span>
);

export default Badge;
