import { useCall } from '../../hooks/useCall';

/** Compact call control shown in the chat header. */
export default function CallBar({ peerId }: { peerId: string | null }) {
  const { status, muted, error, startCall, hangup, toggleMute } = useCall();

  const inCall = status !== 'idle' && status !== 'ringing_in';
  const canCall = !!peerId && peerId.includes('@') && !peerId.startsWith('webhook-');

  if (!inCall) {
    return (
      <button
        type="button"
        disabled={!canCall}
        onClick={() => peerId && startCall(peerId)}
        title={canCall ? 'Start voice call' : 'Voice calls need a user@domain chat'}
        className="w-9 h-9 flex items-center justify-center rounded-lg text-[#71747a] hover:text-[#e8eaed] hover:bg-[#232529] disabled:opacity-30 disabled:hover:bg-transparent transition-colors"
      >
        <span className="material-symbols-outlined text-[20px]">call</span>
      </button>
    );
  }

  const label =
    status === 'ringing_out' ? 'Ringing…' : status === 'connecting' ? 'Connecting…' : 'In call';

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-[#232529]">
      <span className="material-symbols-outlined text-[18px] text-green-400 animate-pulse">call</span>
      <span className="text-xs text-[#e8eaed]">{error ?? label}</span>
      <button
        type="button"
        onClick={toggleMute}
        title={muted ? 'Unmute' : 'Mute'}
        className="w-7 h-7 flex items-center justify-center rounded-md text-[#71747a] hover:text-[#e8eaed] hover:bg-[#0a0a0b]"
      >
        <span className="material-symbols-outlined text-[18px]">{muted ? 'mic_off' : 'mic'}</span>
      </button>
      <button
        type="button"
        onClick={hangup}
        title="Hang up"
        className="w-7 h-7 flex items-center justify-center rounded-md text-white bg-red-600 hover:bg-red-500"
      >
        <span className="material-symbols-outlined text-[18px]">call_end</span>
      </button>
    </div>
  );
}
