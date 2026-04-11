import type { FC } from "react";;

interface SpinnerProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

const sizeMap = {
  sm: 'w-4 h-4 border-2',
  md: 'w-6 h-6 border-2',
  lg: 'w-8 h-8 border-3',
};

const Spinner: FC<SpinnerProps> = ({ size = 'md', className = '' }) => (
  <div
    className={`animate-spin rounded-full border-[var(--accent)] border-t-transparent ${sizeMap[size]} ${className}`}
  />
);

export default Spinner;
