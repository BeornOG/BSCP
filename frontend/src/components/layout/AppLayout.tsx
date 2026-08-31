import type { ReactNode } from "react";
import { useEffect } from "react";
import { NavLink } from "react-router-dom";
import { useProfile } from "../../hooks/useProfile";
import { useIsAdmin } from "../../hooks/useAdmin";
import { useUserActivityPing } from "../../hooks/useActivity";
import { usePushNotifications, useMessageNotifications, initAudioContext } from "../../hooks/useNotifications";
import { useChats } from "../../hooks/useChats";
import { CallProvider } from "../../hooks/useCall";
import IncomingCallModal from "../call/IncomingCallModal";
import GuildRail from "../guild/GuildRail";

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
  const { data: chats } = useChats();

  const totalUnread = chats?.reduce((sum, chat) => sum + chat.unread_count, 0) ?? 0;

  useEffect(() => {
    initAudioContext();
  }, []);

  return (
    <CallProvider>
    <div className="flex h-screen bg-[#0a0a0b]">
      <IncomingCallModal />
      <nav className="w-16 flex flex-col items-center py-4 border-r border-[#232529] bg-[#0a0a0b]">
        <span className="text-xl font-bold text-[var(--accent)] mb-6">A.</span>

        <div className="flex-1 w-full flex flex-col items-center gap-2 overflow-y-auto">
          {navItems.map((item) => (
            <div key={item.path} className="relative">
              <NavLink
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
                <span className="material-symbols-outlined text-[20px]">{item.icon}</span>
              </NavLink>
              {item.path === "/" && totalUnread > 0 && (
                <div className="absolute -top-1 -right-1 bg-red-500 text-white text-xs font-semibold rounded-full w-5 h-5 flex items-center justify-center">
                  {totalUnread > 9 ? "9+" : totalUnread}
                </div>
              )}
            </div>
          ))}

          <div className="w-8 h-px bg-[#232529] my-1" />
          <GuildRail />
          <div className="w-8 h-px bg-[#232529] my-1" />

          {isAdmin && (
            <NavLink
              to="/admin"
              className={({ isActive }) =>
                `w-10 h-10 flex items-center justify-center rounded-xl transition-colors ${
                  isActive
                    ? "bg-[#232529] text-[#e8eaed]"
                    : "text-[#71747a] hover:text-[#e8eaed] hover:bg-[#232529]/50"
                }`
              }
              title="Admin"
            >
              <span className="material-symbols-outlined text-[20px]">admin_panel_settings</span>
            </NavLink>
          )}
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
    </CallProvider>
  );
}
