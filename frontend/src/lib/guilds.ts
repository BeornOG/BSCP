import { api } from './api';

/** Call a channel-server endpoint through our own user server's gateway. */
export function gw<T = unknown>(cs: string, path: string, opts?: RequestInit): Promise<T> {
  return api<T>(`/api/gw/${cs}/${path.replace(/^\//, '')}`, opts);
}

export function gwJson<T = unknown>(
  cs: string,
  path: string,
  method: string,
  body?: unknown,
): Promise<T> {
  return gw<T>(cs, path, {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: body === undefined ? '{}' : JSON.stringify(body),
  });
}

// Permission bits (must match crates/channelserver/src/perms.rs)
export const P = {
  VIEW_CHANNEL: 1 << 0,
  SEND_MESSAGES: 1 << 1,
  MANAGE_MESSAGES: 1 << 2,
  CONNECT: 1 << 3,
  SPEAK: 1 << 4,
  MANAGE_CHANNELS: 1 << 5,
  MANAGE_ROLES: 1 << 6,
  MANAGE_GUILD: 1 << 7,
  KICK_MEMBERS: 1 << 8,
  CREATE_INVITE: 1 << 9,
  ADMINISTRATOR: 1 << 10,
} as const;

export const can = (mask: number, perm: number) =>
  (mask & P.ADMINISTRATOR) !== 0 || (mask & perm) === perm;

export interface GuildSummary {
  channel_server: string;
  guild_id: string;
  name: string | null;
  icon: string | null;
  joined_at: number;
}

export interface GChannel {
  id: string;
  name: string;
  kind: 'text' | 'voice' | 'category';
  parent_id: string | null;
  topic: string | null;
  position: number;
  path: string;
  my_permissions: number;
}

export interface GRole {
  id: string;
  name: string;
  color: string | null;
  position: number;
  permissions: number;
  is_everyone: boolean;
}

export interface GuildDetail {
  id: string;
  name: string;
  icon: string | null;
  owner: string;
  my_permissions: number;
  channels: GChannel[];
  roles: GRole[];
}

export interface GMember {
  user_id: string;
  nickname: string | null;
  joined_at: number;
  roles: string[];
}

export interface GMessage {
  id: string;
  sender: string;
  text: string;
  timestamp: number;
}
