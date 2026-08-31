import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, apiPost } from '../lib/api';
import { gw, gwJson } from '../lib/guilds';
import type {
  GuildDetail,
  GuildSummary,
  GMember,
  GMessage,
  ChannelOverride,
  Webhook,
} from '../lib/guilds';

export function useGuilds() {
  return useQuery<GuildSummary[]>({
    queryKey: ['guilds'],
    queryFn: () => api<GuildSummary[]>('/api/guilds'),
    refetchInterval: 30_000,
  });
}

export function useGuild(cs: string | null, gid: string | null) {
  return useQuery<GuildDetail>({
    queryKey: ['guild', cs, gid],
    queryFn: () => gw<GuildDetail>(cs!, `guilds/${gid}`),
    enabled: !!cs && !!gid,
    refetchInterval: 10_000,
  });
}

export function useGuildMembers(cs: string | null, gid: string | null) {
  return useQuery<GMember[]>({
    queryKey: ['guild-members', cs, gid],
    queryFn: () => gw<GMember[]>(cs!, `guilds/${gid}/members`),
    enabled: !!cs && !!gid,
    refetchInterval: 15_000,
  });
}

export function useChannelMessages(cs: string | null, cid: string | null) {
  return useQuery<GMessage[]>({
    queryKey: ['channel-messages', cs, cid],
    queryFn: () => gw<GMessage[]>(cs!, `channels/${cid}/messages`),
    enabled: !!cs && !!cid,
    refetchInterval: 2000,
  });
}

export function useSendChannelMessage(cs: string, cid: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (text: string) => gwJson(cs, `channels/${cid}/messages`, 'POST', { text }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['channel-messages', cs, cid] }),
  });
}

export function useJoinGuild() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (invite: string) => apiPost<{ ok: boolean; channel_server: string; guild_id: string }>('/api/guilds/join', { invite }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['guilds'] }),
  });
}

export function useCreateGuild() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (v: { channel_server: string; name: string }) =>
      apiPost<{ ok: boolean; channel_server: string; guild_id: string }>('/api/guilds/create', v),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['guilds'] }),
  });
}

export function useLeaveGuild() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (v: { channel_server: string; guild_id: string }) => apiPost('/api/guilds/leave', v),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['guilds'] }),
  });
}

// ── guild admin mutations ──────────────────────────────────────────────

export function useGuildAdmin(cs: string, gid: string) {
  const qc = useQueryClient();
  const inv = () => qc.invalidateQueries({ queryKey: ['guild', cs, gid] });
  return {
    createChannel: (v: { name: string; kind: string; parent_id?: string }) =>
      gwJson(cs, `guilds/${gid}/channels`, 'POST', v).then(inv),
    deleteChannel: (cid: string) => gw(cs, `guilds/${gid}/channels/${cid}`, { method: 'DELETE' }).then(inv),
    createRole: (v: { name: string; permissions?: number; color?: string }) =>
      gwJson(cs, `guilds/${gid}/roles`, 'POST', v).then(inv),
    updateRole: (rid: string, v: Record<string, unknown>) =>
      gwJson(cs, `guilds/${gid}/roles/${rid}`, 'PATCH', v).then(inv),
    deleteRole: (rid: string) => gw(cs, `guilds/${gid}/roles/${rid}`, { method: 'DELETE' }).then(inv),
    setMemberRoles: (uid: string, roles: string[]) =>
      gwJson(cs, `guilds/${gid}/members/${encodeURIComponent(uid)}`, 'PATCH', { roles }).then(() =>
        qc.invalidateQueries({ queryKey: ['guild-members', cs, gid] }),
      ),
    kick: (uid: string) =>
      gw(cs, `guilds/${gid}/members/${encodeURIComponent(uid)}`, { method: 'DELETE' }).then(() =>
        qc.invalidateQueries({ queryKey: ['guild-members', cs, gid] }),
      ),
    setOverride: (cid: string, target: string, v: { target_type: string; allow: number; deny: number }) =>
      gwJson(cs, `channels/${cid}/overrides/${encodeURIComponent(target)}`, 'PUT', v).then(inv),
    listOverrides: (cid: string) => gw<ChannelOverride[]>(cs, `channels/${cid}/overrides`),
    listWebhooks: (cid: string) => gw<Webhook[]>(cs, `channels/${cid}/webhooks`),
    createWebhook: (cid: string, name: string) =>
      gwJson<Webhook>(cs, `channels/${cid}/webhooks`, 'POST', { name }),
    deleteWebhook: (wid: string) => gw(cs, `webhooks/${wid}`, { method: 'DELETE' }),
    regenerateWebhook: (wid: string) =>
      gwJson<{ url: string }>(cs, `webhooks/${wid}/regenerate`, 'POST', {}),
    createInvite: () => gwJson<{ code: string; url: string }>(cs, `guilds/${gid}/invites`, 'POST', {}),
    listInvites: () => gw<{ code: string; url: string; uses: number }[]>(cs, `guilds/${gid}/invites`),
    deleteInvite: (code: string) => gw(cs, `guilds/${gid}/invites/${code}`, { method: 'DELETE' }),
  };
}
