import { useEffect, useMemo, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useGuild, useGuildMembers, useChannelMessages, useSendChannelMessage } from '../hooks/useGuilds';
import { useCall } from '../hooks/useCall';
import { can, P, type GChannel } from '../lib/guilds';
import { useProfile } from '../hooks/useProfile';
import MessageList from '../components/chat/MessageList';
import MessageInput from '../components/chat/MessageInput';
import GuildSettings from '../components/guild/GuildSettings';
import type { Message } from '../types';

export default function GuildPage() {
  const { cs = '', gid = '', cid } = useParams();
  const nav = useNavigate();
  const { data: guild, isLoading, error } = useGuild(cs, gid);
  const { data: members } = useGuildMembers(cs, gid);
  const { data: profile } = useProfile();
  const call = useCall();
  const [settings, setSettings] = useState(false);

  const textChannels = useMemo(() => guild?.channels.filter((c) => c.kind === 'text') ?? [], [guild]);

  // Group visible channels under their category (channels the server hides —
  // e.g. a "Staff" category the member can't view — never arrive here).
  const groups = useMemo(() => {
    const all = guild?.channels ?? [];
    const cats = all.filter((c) => c.kind === 'category').sort((a, b) => a.position - b.position);
    const kids = (parentId: string | null) =>
      all
        .filter((c) => c.kind !== 'category' && (c.parent_id ?? null) === parentId)
        .sort((a, b) => a.position - b.position);
    return [
      { id: null as string | null, name: null as string | null, channels: kids(null) },
      ...cats.map((c) => ({ id: c.id, name: c.name, channels: kids(c.id) })),
    ].filter((g) => g.channels.length);
  }, [guild]);

  const active = useMemo(
    () => guild?.channels.find((c) => c.id === cid) ?? textChannels[0],
    [guild, cid, textChannels],
  );

  useEffect(() => {
    if (guild && active && active.id !== cid) nav(`/g/${cs}/${gid}/${active.id}`, { replace: true });
  }, [guild, active, cid, cs, gid, nav]);

  if (isLoading) return <div className="flex-1 grid place-items-center text-[#71747a]">Loading guild…</div>;
  if (error || !guild)
    return (
      <div className="flex-1 grid place-items-center text-[#71747a]">
        Couldn't open this guild. {error instanceof Error ? error.message : ''}
      </div>
    );

  return (
    <div className="flex h-full">
      <aside className="w-60 flex flex-col border-r border-[#232529] bg-[#0d0d0f]">
        <div className="px-4 h-14 flex items-center justify-between border-b border-[#232529]">
          <span className="font-medium text-[#e8eaed] truncate">{guild.name}</span>
          {can(guild.my_permissions, P.MANAGE_GUILD) && (
            <button onClick={() => setSettings(true)} title="Guild settings" className="text-[#71747a] hover:text-[#e8eaed]">
              <span className="material-symbols-outlined text-[18px]">settings</span>
            </button>
          )}
        </div>
        <div className="flex-1 overflow-y-auto py-2 text-sm">
          {groups.map((g) => (
            <ChannelGroup key={g.id ?? '_root'} label={g.name}>
              {g.channels.map((c) =>
                c.kind === 'voice' ? (
                  <VoiceRow
                    key={c.id}
                    c={c}
                    joined={call.voiceChannel === c.id}
                    members={call.voiceChannel === c.id ? call.peers : []}
                    canConnect={can(c.my_permissions, P.CONNECT)}
                    onJoin={() => (call.voiceChannel === c.id ? call.leaveRoom() : call.joinRoom(cs, c.id))}
                  />
                ) : (
                  <ChannelRow
                    key={c.id}
                    c={c}
                    active={c.id === active?.id}
                    onClick={() => nav(`/g/${cs}/${gid}/${c.id}`)}
                  />
                ),
              )}
            </ChannelGroup>
          ))}
        </div>
        {call.voiceChannel && (
          <div className="px-3 py-2 border-t border-[#232529] flex items-center gap-2 text-xs">
            <span className="material-symbols-outlined text-[16px] text-green-400 animate-pulse">graphic_eq</span>
            <span className="text-[#e8eaed]">Voice connected</span>
            <button onClick={call.toggleMute} className="ml-auto text-[#71747a] hover:text-[#e8eaed]">
              <span className="material-symbols-outlined text-[16px]">{call.muted ? 'mic_off' : 'mic'}</span>
            </button>
            <button onClick={call.leaveRoom} className="text-red-400">
              <span className="material-symbols-outlined text-[16px]">call_end</span>
            </button>
          </div>
        )}
      </aside>

      <section className="flex-1 flex flex-col min-w-0">
        {active?.kind === 'text' ? (
          <TextChannel cs={cs} channel={active} me={profile?.username ?? ''} />
        ) : (
          <div className="flex-1 grid place-items-center text-[#71747a]">Select a text channel</div>
        )}
      </section>

      <aside className="w-52 border-l border-[#232529] bg-[#0d0d0f] overflow-y-auto py-3 px-2 text-sm">
        <p className="px-2 text-xs uppercase tracking-wide text-[#71747a] mb-2">Members — {members?.length ?? 0}</p>
        {(members ?? []).map((m) => (
          <div key={m.user_id} className="px-2 py-1 rounded hover:bg-[#1a1d21] truncate text-[#c7c9cd]">
            {m.nickname || m.user_id.split('@')[0]}
            {m.user_id === guild.owner && <span className="ml-1 text-[10px] text-amber-400">owner</span>}
          </div>
        ))}
      </aside>

      {settings && <GuildSettings cs={cs} gid={gid} onClose={() => setSettings(false)} />}
    </div>
  );
}

function ChannelGroup({ label, children }: { label: string | null; children: React.ReactNode }) {
  return (
    <div className="mb-3">
      {label && (
        <p className="px-3 text-[11px] uppercase tracking-wide text-[#71747a] mb-1">{label}</p>
      )}
      {children}
    </div>
  );
}

function ChannelRow({ c, active, onClick }: { c: GChannel; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={`w-full flex items-center gap-1.5 px-3 py-1 rounded ${
        active ? 'bg-[#232529] text-[#e8eaed]' : 'text-[#a3a5a9] hover:bg-[#1a1d21]'
      }`}
    >
      <span className="text-[#71747a]">#</span>
      <span className="truncate">{c.name}</span>
    </button>
  );
}

function VoiceRow({
  c,
  joined,
  members,
  canConnect,
  onJoin,
}: {
  c: GChannel;
  joined: boolean;
  members: string[];
  canConnect: boolean;
  onJoin: () => void;
}) {
  return (
    <div>
      <button
        onClick={onJoin}
        disabled={!canConnect}
        className={`w-full flex items-center gap-1.5 px-3 py-1 rounded disabled:opacity-40 ${
          joined ? 'text-green-400' : 'text-[#a3a5a9] hover:bg-[#1a1d21]'
        }`}
      >
        <span className="material-symbols-outlined text-[15px]">volume_up</span>
        <span className="truncate">{c.name}</span>
      </button>
      {members.map((m) => (
        <div key={m} className="pl-9 pr-3 py-0.5 text-xs text-[#71747a] truncate">
          {m.split('@')[0]}
        </div>
      ))}
    </div>
  );
}

function TextChannel({ cs, channel, me }: { cs: string; channel: GChannel; me: string }) {
  const { data: msgs } = useChannelMessages(cs, channel.id);
  const send = useSendChannelMessage(cs, channel.id);

  const messages: Message[] = (msgs ?? []).map((m) => ({
    id: m.id,
    sender: m.sender,
    receiver: channel.path,
    text: m.text,
    timestamp: m.timestamp,
    is_read: true,
  }));

  return (
    <>
      <div className="h-14 flex items-center gap-2 px-6 border-b border-[#232529]">
        <span className="text-[#71747a]">#</span>
        <span className="font-medium text-[#e8eaed]">{channel.name}</span>
        {channel.topic && <span className="text-[#71747a] text-xs truncate">— {channel.topic}</span>}
      </div>
      <MessageList
        chatId={channel.id}
        messages={messages}
        currentUser={me}
        isLoading={false}
        onImageClick={() => {}}
      />
      <MessageInput
        onSend={(text) => send.mutate(text)}
        onFileUpload={async () => null}
        disabled={!can(channel.my_permissions, P.SEND_MESSAGES)}
      />
    </>
  );
}
