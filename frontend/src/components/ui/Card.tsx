import type { FC, ReactNode } from "react";;

interface CardProps {
  className?: string;
  children: ReactNode;
}

const Card: FC<CardProps> = ({ className = '', children }) => (
  <div className={`bg-[#141517] border border-[#232529] rounded-2xl overflow-hidden ${className}`}>
    {children}
  </div>
);

interface CardHeaderProps {
  title?: string;
  action?: ReactNode;
  children?: ReactNode;
}

const CardHeader: FC<CardHeaderProps> = ({ title, action, children }) => (
  <div className="flex items-center justify-between px-6 py-4 border-b border-[#232529]">
    {children ?? <h3 className="text-sm font-semibold text-[#e8eaed]">{title}</h3>}
    {action}
  </div>
);

interface CardContentProps {
  className?: string;
  children: ReactNode;
}

const CardContent: FC<CardContentProps> = ({ className = '', children }) => (
  <div className={`px-6 py-4 ${className}`}>{children}</div>
);

export { Card, CardHeader, CardContent };
export default Card;
