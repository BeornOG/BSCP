import type { FC, ReactNode } from "react";;

interface BadgeProps {
  variant?: 'default' | 'success' | 'warning' | 'danger' | 'unread';
  children: ReactNode;
}

const variantStyles = {
  default: 'bg-[#232529] text-[#71747a]',
  success: 'bg-green-500/15 text-green-400',
  warning: 'bg-amber-500/15 text-amber-400',
  danger: 'bg-red-500/15 text-red-400',
  unread: 'bg-red-500 text-white',
};

const Badge: FC<BadgeProps> = ({ variant = 'default', children }) => (
  <span
    className={`inline-flex items-center justify-center rounded-full min-w-5 h-5 text-xs font-semibold ${variantStyles[variant as keyof typeof variantStyles]}`}
  >
    {children}
  </span>
);

export default Badge;
