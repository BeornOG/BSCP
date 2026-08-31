import { useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useJoinGuild } from '../hooks/useGuilds';

/** Landing for `/join?invite=…` deep links from a channel server. */
export default function JoinGuildPage() {
  const [params] = useSearchParams();
  const invite = params.get('invite') ?? '';
  const join = useJoinGuild();
  const nav = useNavigate();
  const [err, setErr] = useState('');
  const [done, setDone] = useState(false);

  useEffect(() => {
    if (!invite) {
      setErr('missing invite');
    }
  }, [invite]);

  const accept = async () => {
    setErr('');
    try {
      const r = await join.mutateAsync(invite);
      if (!r.ok) throw new Error('could not join with that invite');
      setDone(true);
      nav(`/g/${r.channel_server}/${r.guild_id}`, { replace: true });
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'failed');
    }
  };

  return (
    <div className="flex-1 grid place-items-center">
      <div className="w-80 rounded-2xl bg-[#151517] border border-[#232529] p-6 text-center">
        <span className="material-symbols-outlined text-[36px] text-[var(--accent)]">group_add</span>
        <p className="mt-2 text-[#e8eaed] font-medium">Join a guild</p>
        <p className="text-[#71747a] text-xs break-all mt-1">{invite || '—'}</p>
        {err && <p className="text-red-400 text-xs mt-2">{err}</p>}
        <button
          className="mt-4 w-full py-2 rounded-lg bg-[var(--accent)] text-white text-sm disabled:opacity-50"
          disabled={!invite || join.isPending || done}
          onClick={accept}
        >
          {join.isPending ? 'Joining…' : 'Accept invite'}
        </button>
      </div>
    </div>
  );
}
