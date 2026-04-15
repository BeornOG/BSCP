import type { ReactNode } from "react";
import { NavLink } from "react-router-dom";
import { useProfile } from "../../hooks/useProfile";
import { useIsAdmin } from "../../hooks/useAdmin";
import { useUserActivityPing } from "../../hooks/useActivity";
import { usePushNotifications, useMessageNotifications } from "../../hooks/useNotifications";

interface AppLayoutProps {
  children: ReactNode;
}

const navItems = [
  { path: "/", icon: "chat", label: "Messages" },
];

export default function AppLayout({ children }: AppLayoutProps) {
  usePushNotifications();
  useMessageNotifications();
  useUserActivityPing();
  const { data: profile, isLoading } = useProfile();
  const { data: isAdmin } = useIsAdmin();

  const allNavItems = isAdmin
    ? [...navItems, { path: "/admin", icon: "admin_panel_settings", label: "Admin" }]
    : navItems;

  return (
    <div className="flex h-screen bg-[#0a0a0b]">
      <nav className="w-16 flex flex-col items-center py-4 border-r border-[#232529] bg-[#0a0a0b]">
        <span className="text-xl font-bold text-[var(--accent)] mb-8">A.</span>

        <div className="flex-1 flex flex-col items-center gap-2">
          {allNavItems.map((item) => (
            <NavLink
              key={item.path}
              to={item.path}
              end={item.path === "/"}
              className={({ isActive }) =>
                `w-10 h-10 flex items-center justify-center rounded-xl transition-colors ${
                  isActive
                    ? "bg-[#232529] text-[#e8eaed]"
                    : "text-[#71747a] hover:text-[#e8eaed] hover:bg-[#232529]/50"
                }`
              }
              title={item.label}
            >
              <span className="material-symbols-outlined text-[20px]">
                {item.icon}
              </span>
            </NavLink>
          ))}
        </div>

        <div className="flex flex-col items-center gap-3">
          <NavLink
            to="/settings"
            className={({ isActive }) =>
              `w-10 h-10 flex items-center justify-center rounded-xl transition-colors ${
                isActive
                  ? "bg-[#232529] text-[#e8eaed]"
                  : "text-[#71747a] hover:text-[#e8eaed] hover:bg-[#232529]/50"
              }`
            }
            title="Settings"
          >
            <span className="material-symbols-outlined text-[20px]">settings</span>
          </NavLink>

          <div className="w-8 h-8 rounded-full overflow-hidden bg-[#232529] flex items-center justify-center">
            {!isLoading && profile?.profile_pic ? (
              <img
                src={profile.profile_pic}
                alt={profile.display_name ?? "User"}
                className="w-full h-full object-cover"
              />
            ) : (
              <span className="material-symbols-outlined text-[16px] text-[#71747a]">
                person
              </span>
            )}
          </div>
        </div>
      </nav>

      <main className="flex-1 overflow-hidden">{children}</main>
    </div>
  );
}
