import type { ReactNode } from "react";

interface AuthLayoutProps {
  title: string;
  subtitle?: string;
  children: ReactNode;
}

export default function AuthLayout({ title, subtitle, children }: AuthLayoutProps) {
  return (
    <div className="min-h-screen flex items-center justify-center bg-[#0a0a0b] relative overflow-hidden">
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--accent)_0%,_transparent_50%)] opacity-[0.03]" />

      <div className="relative w-full max-w-md mx-4 bg-[#141517] border border-[#232529] rounded-2xl p-8">
        <div className="flex flex-col items-center mb-8">
          <span className="text-3xl font-bold text-[var(--accent)] mb-6">A.</span>
          <h1 className="text-2xl font-semibold text-[#e8eaed] text-center">{title}</h1>
          {subtitle && (
            <p className="mt-2 text-sm text-[#71747a] text-center">{subtitle}</p>
          )}
        </div>

        {children}
      </div>
    </div>
  );
}
