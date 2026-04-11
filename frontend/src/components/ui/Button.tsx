import { type ButtonHTMLAttributes, type FC, type ReactNode } from 'react';
import Spinner from './Spinner';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  size?: 'sm' | 'md' | 'lg';
  loading?: boolean;
  icon?: ReactNode;
}

const variantStyles = {
  primary: 'bg-[var(--accent)] text-white hover:opacity-90',
  secondary: 'bg-[#141517] border border-[#232529] text-[#e8eaed] hover:bg-[#1c1d21]',
  ghost: 'bg-transparent text-[#e8eaed] hover:bg-[#141517]',
  danger: 'bg-red-600 text-white hover:bg-red-700',
};

const sizeStyles = {
  sm: 'px-3 py-1.5 text-sm',
  md: 'px-4 py-2 text-sm',
  lg: 'px-6 py-3 text-base',
};

const Button: FC<ButtonProps> = ({
  variant = 'primary',
  size = 'md',
  loading = false,
  icon,
  children,
  disabled,
  className = '',
  ...props
}) => (
  <button
    className={`inline-flex items-center justify-center gap-2 rounded-xl font-medium transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed ${variantStyles[variant]} ${sizeStyles[size]} ${className}`}
    disabled={disabled || loading}
    {...props}
  >
    {loading ? <Spinner size="sm" /> : icon}
    {children}
  </button>
);

export default Button;
