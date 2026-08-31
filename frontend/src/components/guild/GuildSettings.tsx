import { useEffect, useState } from 'react';
import { useGuild, useGuildMembers, useGuildAdmin } from '../../hooks/useGuilds';
import { P, can } from '../../lib/guilds';

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

type Tab = 'channels' | 'roles' | 'members' | 'invites';

export default function GuildSettings({ cs, gid, onClose }: { cs: string; gid: string; onClose: () => void }) {
  const [tab, setTab] = useState<Tab>('channels');
  const { data: guild } = useGuild(cs, gid);
  const admin = useGuildAdmin(cs, gid);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={onClose}>
      <div
        className="w-[640px] max-h-[80vh] rounded-2xl bg-[#151517] border border-[#232529] flex overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <nav className="w-40 shrink-0 bg-[#0f0f11] p-3 text-sm">
          {(['channels', 'roles', 'members', 'invites'] as Tab[]).map((t) => (
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
          ) : (
            <InvitesTab admin={admin} />
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
  return (
    <div>
      <h3 className="font-medium text-[#e8eaed] mb-3">Channels</h3>
      {guild.channels.map((c) => (
        <div key={c.id} className="flex items-center gap-2 py-1">
          <span className="text-[#71747a]">{c.kind === 'voice' ? '🔊' : '#'}</span>
          <span className="flex-1 truncate text-[#c7c9cd]">{c.name}</span>
          <button className="text-red-400 text-xs" onClick={() => admin.deleteChannel(c.id)}>
            delete
          </button>
        </div>
      ))}
      <div className="flex gap-2 mt-3">
        <input
          className="flex-1 px-2 py-1 rounded bg-[#0f0f11] border border-[#333]"
          placeholder="new channel name"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <select className="bg-[#0f0f11] border border-[#333] rounded px-2" value={kind} onChange={(e) => setKind(e.target.value)}>
          <option value="text">text</option>
          <option value="voice">voice</option>
          <option value="category">category</option>
        </select>
        <button
          className="px-3 rounded bg-[var(--accent)] text-white"
          onClick={() => {
            if (name.trim()) admin.createChannel({ name: name.trim(), kind }).then(() => setName(''));
          }}
        >
          add
        </button>
      </div>
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
