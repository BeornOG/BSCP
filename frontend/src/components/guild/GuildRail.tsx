import { useState } from 'react';
import { NavLink, useNavigate } from 'react-router-dom';
import { useGuilds, useCreateGuild, useJoinGuild } from '../../hooks/useGuilds';

export default function GuildRail() {
  const { data: guilds } = useGuilds();
  const [open, setOpen] = useState(false);

  return (
    <>
      {(guilds ?? []).map((g) => (
        <NavLink
          key={`${g.channel_server}/${g.guild_id}`}
          to={`/g/${g.channel_server}/${g.guild_id}`}
          title={g.name ?? g.guild_id}
          className={({ isActive }) =>
            `w-10 h-10 rounded-xl flex items-center justify-center text-xs font-semibold overflow-hidden transition-colors ${
              isActive
                ? 'bg-[var(--accent)] text-white'
                : 'bg-[#1a1d21] text-[#c7c9cd] hover:bg-[var(--accent)]/70'
            }`
          }
        >
          {g.icon ? (
            <img src={g.icon} alt="" className="w-full h-full object-cover" />
          ) : (
            (g.name ?? '?').slice(0, 2).toUpperCase()
          )}
        </NavLink>
      ))}

      <button
        onClick={() => setOpen(true)}
        className="w-10 h-10 rounded-xl bg-[#1a1d21] text-green-400 text-lg hover:bg-green-600 hover:text-white"
        title="Add a guild"
      >
        +
      </button>

      {open && <GuildDialog onClose={() => setOpen(false)} />}
    </>
  );
}

function GuildDialog({ onClose }: { onClose: () => void }) {
  const nav = useNavigate();
  const create = useCreateGuild();
  const join = useJoinGuild();
  const [mode, setMode] = useState<'create' | 'join'>('create');
  const [cs, setCs] = useState('');
  const [name, setName] = useState('');
  const [invite, setInvite] = useState('');
  const [err, setErr] = useState('');

  const submit = async () => {
    setErr('');
    try {
      if (mode === 'create') {
        const r = await create.mutateAsync({ channel_server: cs.trim(), name: name.trim() });
        if (!r.ok) throw new Error('server refused — is your account on its guild-creator allowlist?');
        nav(`/g/${r.channel_server}/${r.guild_id}`);
      } else {
        const r = await join.mutateAsync(invite.trim());
        if (!r.ok) throw new Error('could not join with that invite');
        nav(`/g/${r.channel_server}/${r.guild_id}`);
      }
      onClose();
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'failed');
    }
  };

  const tab = (m: 'create' | 'join', label: string) => (
    <button
      onClick={() => {
        setMode(m);
        setErr('');
      }}
      className={`flex-1 py-1.5 text-sm rounded-lg ${
        mode === m ? 'bg-[var(--accent)] text-white' : 'bg-[#0f0f11] text-[#a3a5a9]'
      }`}
    >
      {label}
    </button>
  );

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/60" onClick={onClose}>
      <div
        className="w-80 rounded-2xl bg-[#151517] border border-[#232529] p-5"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex gap-2 mb-4">
          {tab('create', 'Create')}
          {tab('join', 'Join')}
        </div>

        {mode === 'create' ? (
          <>
            <input
              autoFocus
              className="w-full mb-2 px-3 py-2 rounded-lg bg-[#0f0f11] border border-[#333] text-sm outline-none focus:border-[var(--accent)]"
              placeholder="channel server — e.g. localhost:6060"
              value={cs}
              onChange={(e) => setCs(e.target.value)}
            />
            <input
              className="w-full mb-2 px-3 py-2 rounded-lg bg-[#0f0f11] border border-[#333] text-sm outline-none focus:border-[var(--accent)]"
              placeholder="guild name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && submit()}
            />
          </>
        ) : (
          <input
            autoFocus
            className="w-full mb-2 px-3 py-2 rounded-lg bg-[#0f0f11] border border-[#333] text-sm outline-none focus:border-[var(--accent)]"
            placeholder="invite link (https://…/invite/CODE)"
            value={invite}
            onChange={(e) => setInvite(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && submit()}
          />
        )}

        {err && <p className="text-red-400 text-xs mb-2">{err}</p>}

        <div className="flex gap-2 justify-end mt-3">
          <button className="px-3 py-1.5 text-sm text-[#71747a] hover:text-[#e8eaed]" onClick={onClose}>
            Cancel
          </button>
          <button
            className="px-4 py-1.5 text-sm rounded-lg bg-[var(--accent)] text-white disabled:opacity-50"
            disabled={create.isPending || join.isPending}
            onClick={submit}
          >
            {create.isPending || join.isPending ? '…' : mode === 'create' ? 'Create' : 'Join'}
          </button>
        </div>
      </div>
    </div>
  );
}
