import { useEffect, useState } from 'react';
import { useGuild, useGuildMembers, useGuildAdmin } from '../../hooks/useGuilds';
import { P, can } from '../../lib/guilds';
import type { ChannelOverride, Webhook } from '../../lib/guilds';

const PERM_LABELS: [number, string][] = [
  [P.VIEW_CHANNEL, 'View channels'],
  [P.SEND_MESSAGES, 'Send messages'],
  [P.MANAGE_MESSAGES, 'Manage messages'],
  [P.CONNECT, 'Connect to voice'],
  [P.SPEAK, 'Speak'],
  [P.MANAGE_CHANNELS, 'Manage channels'],
  [P.MANAGE_ROLES, 'Manage roles'],
  [P.MANAGE_GUILD, 'Manage guild'],
  [P.KICK_MEMBERS, 'Kick members'],
  [P.CREATE_INVITE, 'Create invites'],
  [P.ADMINISTRATOR, 'Administrator'],
];

// The subset that makes sense as a per-channel / per-category override.
const OVERRIDE_PERMS: [number, string][] = [
  [P.VIEW_CHANNEL, 'View'],
  [P.SEND_MESSAGES, 'Send'],
  [P.MANAGE_MESSAGES, 'Manage msgs'],
  [P.CONNECT, 'Connect'],
  [P.SPEAK, 'Speak'],
];

type Tab = 'channels' | 'roles' | 'members' | 'invites' | 'webhooks';
const TABS: Tab[] = ['channels', 'roles', 'members', 'invites', 'webhooks'];

export default function GuildSettings({ cs, gid, onClose }: { cs: string; gid: string; onClose: () => void }) {
  const [tab, setTab] = useState<Tab>('channels');
  const { data: guild } = useGuild(cs, gid);
  const admin = useGuildAdmin(cs, gid);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={onClose}>
      <div
        className="w-[680px] max-h-[80vh] rounded-2xl bg-[#151517] border border-[#232529] flex overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <nav className="w-40 shrink-0 bg-[#0f0f11] p-3 text-sm">
          {TABS.map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={`w-full text-left px-3 py-1.5 rounded capitalize ${
                tab === t ? 'bg-[#232529] text-[#e8eaed]' : 'text-[#a3a5a9] hover:bg-[#1a1d21]'
              }`}
            >
              {t}
            </button>
          ))}
        </nav>
        <div className="flex-1 p-5 overflow-y-auto text-sm">
          {!guild ? (
            <p className="text-[#71747a]">Loading…</p>
          ) : tab === 'channels' ? (
            <ChannelsTab guild={guild} admin={admin} />
          ) : tab === 'roles' ? (
            <RolesTab guild={guild} admin={admin} />
          ) : tab === 'members' ? (
            <MembersTab cs={cs} gid={gid} guild={guild} admin={admin} />
          ) : tab === 'invites' ? (
            <InvitesTab admin={admin} />
          ) : (
            <WebhooksTab guild={guild} admin={admin} />
          )}
        </div>
      </div>
    </div>
  );
}

type Admin = ReturnType<typeof useGuildAdmin>;
type Guild = NonNullable<ReturnType<typeof useGuild>['data']>;

function ChannelsTab({ guild, admin }: { guild: Guild; admin: Admin }) {
  const [name, setName] = useState('');
  const [kind, setKind] = useState('text');
  const [parent, setParent] = useState('');
  const [permsFor, setPermsFor] = useState<string | null>(null);
  const categories = guild.channels.filter((c) => c.kind === 'category');

  return (
    <div>
      <h3 className="font-medium text-[#e8eaed] mb-3">Channels</h3>
      {guild.channels.map((c) => (
        <div key={c.id}>
          <div className="flex items-center gap-2 py-1">
            <span className="text-[#71747a]">
              {c.kind === 'voice' ? '🔊' : c.kind === 'category' ? '📁' : '#'}
            </span>
            <span className="flex-1 truncate text-[#c7c9cd]">
              {c.name}
              {c.parent_id && (
                <span className="text-[#5b5e63] text-xs">
                  {' '}
                  · {categories.find((p) => p.id === c.parent_id)?.name ?? '—'}
                </span>
              )}
            </span>
            <button
              className="text-xs text-[#71747a] hover:text-[#e8eaed]"
              onClick={() => setPermsFor(permsFor === c.id ? null : c.id)}
            >
              {permsFor === c.id ? 'hide perms' : 'perms'}
            </button>
            <button className="text-red-400 text-xs" onClick={() => admin.deleteChannel(c.id)}>
              delete
            </button>
          </div>
          {permsFor === c.id && <ChannelPermsPanel guild={guild} admin={admin} cid={c.id} />}
        </div>
      ))}

      <div className="flex flex-wrap gap-2 mt-4">
        <input
          className="flex-1 min-w-[8rem] px-2 py-1 rounded bg-[#0f0f11] border border-[#333]"
          placeholder="new channel name"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <select
          className="bg-[#0f0f11] border border-[#333] rounded px-2"
          value={kind}
          onChange={(e) => setKind(e.target.value)}
        >
          <option value="text">text</option>
          <option value="voice">voice</option>
          <option value="category">category</option>
        </select>
        <select
          className="bg-[#0f0f11] border border-[#333] rounded px-2 max-w-[9rem]"
          value={parent}
          onChange={(e) => setParent(e.target.value)}
          disabled={kind === 'category'}
        >
          <option value="">no category</option>
          {categories.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
        <button
          className="px-3 rounded bg-[var(--accent)] text-white"
          onClick={() => {
            if (!name.trim()) return;
            admin
              .createChannel({
                name: name.trim(),
                kind,
                parent_id: kind === 'category' || !parent ? undefined : parent,
              })
              .then(() => setName(''));
          }}
        >
          add
        </button>
      </div>
      <p className="text-xs text-[#5b5e63] mt-2">
        Tip: make a “Staff” category, open its <b>perms</b>, deny <i>View</i> for @everyone and allow it for
        the staff role. Channels inside inherit it.
      </p>
    </div>
  );
}

function ChannelPermsPanel({ guild, admin, cid }: { guild: Guild; admin: Admin; cid: string }) {
  const [ov, setOv] = useState<ChannelOverride[] | null>(null);
  const reload = () => admin.listOverrides(cid).then(setOv).catch(() => setOv([]));
  useEffect(() => {
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cid]);

  const roleOv = (rid: string) =>
    ov?.find((o) => o.target_type === 'role' && o.target_id === rid) ?? { allow: 0, deny: 0 };

  const setState = async (rid: string, bit: number, next: 'inherit' | 'allow' | 'deny') => {
    const cur = roleOv(rid);
    let allow = cur.allow & ~bit;
    let deny = cur.deny & ~bit;
    if (next === 'allow') allow |= bit;
    if (next === 'deny') deny |= bit;
    await admin.setOverride(cid, rid, { target_type: 'role', allow, deny });
    reload();
  };

  if (!ov) return <p className="pl-6 py-1 text-xs text-[#5b5e63]">Loading permissions…</p>;

  return (
    <div className="ml-6 mb-2 rounded-lg bg-[#0f0f11] border border-[#232529] p-2 overflow-x-auto">
      <table className="text-xs">
        <thead>
          <tr className="text-[#5b5e63]">
            <th className="text-left pr-3 font-normal">role</th>
            {OVERRIDE_PERMS.map(([, label]) => (
              <th key={label} className="px-1 font-normal">
                {label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {guild.roles.map((r) => {
            const cur = roleOv(r.id);
            return (
              <tr key={r.id} className="text-[#c7c9cd]">
                <td className="pr-3 py-0.5 whitespace-nowrap">{r.name}</td>
                {OVERRIDE_PERMS.map(([bit]) => {
                  const on = cur.allow & bit ? 'allow' : cur.deny & bit ? 'deny' : 'inherit';
                  return (
                    <td key={bit} className="px-1 text-center">
                      <div className="inline-flex rounded overflow-hidden border border-[#333]">
                        {(['deny', 'inherit', 'allow'] as const).map((s) => (
                          <button
                            key={s}
                            title={s}
                            onClick={() => setState(r.id, bit, s)}
                            className={`w-5 h-5 leading-5 ${
                              on === s
                                ? s === 'allow'
                                  ? 'bg-green-600 text-white'
                                  : s === 'deny'
                                    ? 'bg-red-600 text-white'
                                    : 'bg-[#333] text-[#e8eaed]'
                                : 'text-[#5b5e63] hover:bg-[#1a1d21]'
                            }`}
                          >
                            {s === 'allow' ? '✓' : s === 'deny' ? '✕' : '∅'}
                          </button>
                        ))}
                      </div>
                    </td>
                  );
                })}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function RolesTab({ guild, admin }: { guild: Guild; admin: Admin }) {
  const [sel, setSel] = useState<string | null>(guild.roles[0]?.id ?? null);
  const role = guild.roles.find((r) => r.id === sel);
  const [name, setName] = useState('');

  return (
    <div className="flex gap-4">
      <div className="w-40 shrink-0">
        <h3 className="font-medium text-[#e8eaed] mb-2">Roles</h3>
        {guild.roles.map((r) => (
          <button
            key={r.id}
            onClick={() => setSel(r.id)}
            className={`block w-full text-left px-2 py-1 rounded ${sel === r.id ? 'bg-[#232529]' : 'hover:bg-[#1a1d21]'}`}
          >
            {r.name}
          </button>
        ))}
        <div className="flex gap-1 mt-2">
          <input
            className="flex-1 px-2 py-1 rounded bg-[#0f0f11] border border-[#333] text-xs"
            placeholder="new role"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <button
            className="px-2 rounded bg-[var(--accent)] text-white text-xs"
            onClick={() => name.trim() && admin.createRole({ name: name.trim() }).then(() => setName(''))}
          >
            +
          </button>
        </div>
      </div>
      {role && (
        <div className="flex-1">
          <p className="text-[#e8eaed] mb-2">
            {role.name} {role.is_everyone && <span className="text-xs text-[#71747a]">(base role)</span>}
          </p>
          {PERM_LABELS.map(([bit, label]) => (
            <label key={bit} className="flex items-center gap-2 py-0.5 text-[#c7c9cd]">
              <input
                type="checkbox"
                checked={can(role.permissions, bit)}
                onChange={(e) => {
                  const p = e.target.checked ? role.permissions | bit : role.permissions & ~bit;
                  admin.updateRole(role.id, { permissions: p });
                }}
              />
              {label}
            </label>
          ))}
          {!role.is_everyone && (
            <button className="mt-3 text-red-400 text-xs" onClick={() => admin.deleteRole(role.id)}>
              delete role
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function MembersTab({ cs, gid, guild, admin }: { cs: string; gid: string; guild: Guild; admin: Admin }) {
  const { data: members } = useGuildMembers(cs, gid);
  const assignable = guild.roles.filter((r) => !r.is_everyone);
  return (
    <div>
      <h3 className="font-medium text-[#e8eaed] mb-3">Members</h3>
      {(members ?? []).map((m) => (
        <div key={m.user_id} className="py-2 border-b border-[#1f1f22]">
          <div className="flex items-center">
            <span className="flex-1 text-[#c7c9cd]">{m.user_id}</span>
            {m.user_id !== guild.owner && (
              <button className="text-red-400 text-xs" onClick={() => admin.kick(m.user_id)}>
                kick
              </button>
            )}
          </div>
          <div className="flex flex-wrap gap-2 mt-1">
            {assignable.map((r) => {
              const has = m.roles.includes(r.id);
              return (
                <button
                  key={r.id}
                  onClick={() => {
                    const next = has ? m.roles.filter((x) => x !== r.id) : [...m.roles, r.id];
                    admin.setMemberRoles(m.user_id, next);
                  }}
                  className={`text-xs px-2 py-0.5 rounded-full border ${
                    has ? 'bg-[var(--accent)] border-transparent text-white' : 'border-[#333] text-[#71747a]'
                  }`}
                >
                  {r.name}
                </button>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

function InvitesTab({ admin }: { admin: Admin }) {
  const [invites, setInvites] = useState<{ code: string; url: string; uses: number }[]>([]);
  const [copied, setCopied] = useState('');
  const refresh = () => admin.listInvites().then(setInvites).catch(() => {});
  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return (
    <div>
      <h3 className="font-medium text-[#e8eaed] mb-3">Invites</h3>
      {invites.map((i) => (
        <div key={i.code} className="flex items-center gap-2 py-1 text-[#c7c9cd]">
          <code className="flex-1 truncate text-xs">{i.url}</code>
          <span className="text-xs text-[#71747a]">{i.uses} uses</span>
          <button
            className="text-xs text-[var(--accent)]"
            onClick={() => {
              navigator.clipboard.writeText(i.url);
              setCopied(i.code);
            }}
          >
            {copied === i.code ? 'copied' : 'copy'}
          </button>
          <button className="text-xs text-red-400" onClick={() => admin.deleteInvite(i.code).then(refresh)}>
            ✕
          </button>
        </div>
      ))}
      <button
        className="mt-3 px-3 py-1.5 rounded bg-[var(--accent)] text-white text-sm"
        onClick={() => admin.createInvite().then(refresh)}
      >
        Create invite link
      </button>
    </div>
  );
}

function WebhooksTab({ guild, admin }: { guild: Guild; admin: Admin }) {
  const textChannels = guild.channels.filter((c) => c.kind === 'text');
  const [cid, setCid] = useState(textChannels[0]?.id ?? '');
  const [hooks, setHooks] = useState<Webhook[]>([]);
  const [name, setName] = useState('');
  const [copied, setCopied] = useState('');

  const refresh = (c = cid) => {
    if (!c) return setHooks([]);
    admin.listWebhooks(c).then(setHooks).catch(() => setHooks([]));
  };
  useEffect(() => {
    refresh(cid);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cid]);

  if (!textChannels.length) {
    return <p className="text-[#71747a]">Create a text channel first.</p>;
  }

  return (
    <div>
      <h3 className="font-medium text-[#e8eaed] mb-3">Webhooks</h3>
      <select
        className="bg-[#0f0f11] border border-[#333] rounded px-2 py-1 mb-3"
        value={cid}
        onChange={(e) => setCid(e.target.value)}
      >
        {textChannels.map((c) => (
          <option key={c.id} value={c.id}>
            #{c.name}
          </option>
        ))}
      </select>

      {hooks.map((h) => (
        <div key={h.id} className="py-2 border-b border-[#1f1f22]">
          <div className="flex items-center gap-2">
            <span className="flex-1 text-[#c7c9cd]">{h.name}</span>
            <button
              className="text-xs text-[var(--accent)]"
              onClick={() => {
                navigator.clipboard.writeText(h.url);
                setCopied(h.id);
              }}
            >
              {copied === h.id ? 'copied' : 'copy URL'}
            </button>
            <button
              className="text-xs text-[#71747a] hover:text-[#e8eaed]"
              onClick={() => admin.regenerateWebhook(h.id).then(() => refresh())}
            >
              regenerate
            </button>
            <button
              className="text-xs text-red-400"
              onClick={() => admin.deleteWebhook(h.id).then(() => refresh())}
            >
              delete
            </button>
          </div>
          <code className="block mt-1 text-[10px] text-[#5b5e63] truncate">{h.url}</code>
        </div>
      ))}
      {!hooks.length && <p className="text-[#5b5e63] text-xs py-2">No webhooks for this channel yet.</p>}

      <div className="flex gap-2 mt-3">
        <input
          className="flex-1 px-2 py-1 rounded bg-[#0f0f11] border border-[#333]"
          placeholder="webhook name (e.g. GitHub)"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <button
          className="px-3 rounded bg-[var(--accent)] text-white"
          onClick={() => {
            if (name.trim() && cid)
              admin.createWebhook(cid, name.trim()).then(() => {
                setName('');
                refresh();
              });
          }}
        >
          create
        </button>
      </div>
      <p className="text-xs text-[#5b5e63] mt-2">
        POST JSON <code>{'{ "content": "…", "username": "…" }'}</code> to the URL to post a message.
      </p>
    </div>
  );
}
