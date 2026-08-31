import { useState } from 'react';
import { NavLink, useNavigate } from 'react-router-dom';
import { useGuilds, useCreateGuild, useJoinGuild } from '../../hooks/useGuilds';

export default function GuildRail() {
  const { data: guilds } = useGuilds();
  const [menu, setMenu] = useState(false);
  const [mode, setMode] = useState<null | 'create' | 'join'>(null);

  return (
    <div className="w-16 flex flex-col items-center gap-2 py-3 border-r border-[#232529] bg-[#050506]">
      {(guilds ?? []).map((g) => (
        <NavLink
          key={`${g.channel_server}/${g.guild_id}`}
          to={`/g/${g.channel_server}/${g.guild_id}`}
          title={g.name ?? g.guild_id}
          className={({ isActive }) =>
            `w-11 h-11 rounded-2xl flex items-center justify-center text-sm font-semibold overflow-hidden transition-all ${
              isActive
                ? 'bg-[var(--accent)] text-white rounded-xl'
                : 'bg-[#1a1d21] text-[#c7c9cd] hover:rounded-xl hover:bg-[var(--accent)]/70'
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

      <div className="relative">
        <button
          onClick={() => setMenu((m) => !m)}
          className="w-11 h-11 rounded-2xl bg-[#1a1d21] text-green-400 text-xl hover:rounded-xl hover:bg-green-600 hover:text-white"
          title="Add a guild"
        >
          +
        </button>
        {menu && (
          <div className="absolute left-14 top-0 z-20 w-40 rounded-lg bg-[#151517] border border-[#232529] p-1 text-sm">
            <button
              className="w-full text-left px-3 py-2 rounded hover:bg-[#232529]"
              onClick={() => {
                setMenu(false);
                setMode('join');
              }}
            >
              Join a guild
            </button>
            <button
              className="w-full text-left px-3 py-2 rounded hover:bg-[#232529]"
              onClick={() => {
                setMenu(false);
                setMode('create');
              }}
            >
              Create a guild
            </button>
          </div>
        )}
      </div>

      {mode && <GuildDialog mode={mode} onClose={() => setMode(null)} />}
    </div>
  );
}

function GuildDialog({ mode, onClose }: { mode: 'create' | 'join'; onClose: () => void }) {
  const nav = useNavigate();
  const create = useCreateGuild();
  const join = useJoinGuild();
  const [cs, setCs] = useState('');
  const [name, setName] = useState('');
  const [invite, setInvite] = useState('');
  const [err, setErr] = useState('');

  const submit = async () => {
    setErr('');
    try {
      if (mode === 'create') {
        const r = await create.mutateAsync({ channel_server: cs.trim(), name: name.trim() });
        if (!r.ok) throw new Error('server refused (are you on the allowlist?)');
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

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div className="w-80 rounded-2xl bg-[#151517] border border-[#232529] p-5" onClick={(e) => e.stopPropagation()}>
        <h2 className="text-[#e8eaed] font-medium mb-3">{mode === 'create' ? 'Create a guild' : 'Join a guild'}</h2>
        {mode === 'create' ? (
          <>
            <input
              className="w-full mb-2 px-3 py-2 rounded-lg bg-[#0f0f11] border border-[#333] text-sm"
              placeholder="channel server (e.g. chat.example.com)"
              value={cs}
              onChange={(e) => setCs(e.target.value)}
            />
            <input
              className="w-full mb-2 px-3 py-2 rounded-lg bg-[#0f0f11] border border-[#333] text-sm"
              placeholder="guild name"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </>
        ) : (
          <input
            className="w-full mb-2 px-3 py-2 rounded-lg bg-[#0f0f11] border border-[#333] text-sm"
            placeholder="invite link"
            value={invite}
            onChange={(e) => setInvite(e.target.value)}
          />
        )}
        {err && <p className="text-red-400 text-xs mb-2">{err}</p>}
        <div className="flex gap-2 justify-end mt-2">
          <button className="px-3 py-1.5 text-sm text-[#71747a] hover:text-[#e8eaed]" onClick={onClose}>
            Cancel
          </button>
          <button
            className="px-4 py-1.5 text-sm rounded-lg bg-[var(--accent)] text-white disabled:opacity-50"
            disabled={create.isPending || join.isPending}
            onClick={submit}
          >
            {mode === 'create' ? 'Create' : 'Join'}
          </button>
        </div>
      </div>
    </div>
  );
}
