import type { FC } from 'react';

interface AvatarProps {
  src?: string | null;
  fallback?: string;
  name?: string;
  size?: 'sm' | 'md' | 'lg' | 'xl';
  online?: boolean;
}

const sizeMap = {
  sm: 'w-8 h-8 text-xs',
  md: 'w-10 h-10 text-sm',
  lg: 'w-12 h-12 text-base',
  xl: 'w-20 h-20 text-2xl',
};

const dotSizeMap = {
  sm: 'w-2 h-2',
  md: 'w-2.5 h-2.5',
  lg: 'w-3 h-3',
  xl: 'w-4 h-4',
};

const Avatar: FC<AvatarProps> = ({ src, fallback, name, size = 'md', online }) => (
  <div className="relative inline-flex shrink-0">
    {src && src !== '' ? (
      <img
        src={src}
        alt=""
        className={`rounded-full object-cover ${sizeMap[size]}`}
      />
    ) : (
      <div
        className={`flex items-center justify-center rounded-full bg-[#232529] font-medium text-[#e8eaed] ${sizeMap[size]}`}
      >
        {fallback || (name ? name.charAt(0).toUpperCase() : null) || (
          <span className="material-symbols-outlined text-[inherit]">person</span>
        )}
      </div>
    )}
    {online && (
      <span
        className={`absolute bottom-0 right-0 rounded-full border-2 border-[#0a0a0b] bg-green-500 ${dotSizeMap[size]}`}
      />
    )}
  </div>
);

export default Avatar;
