import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';

/** Minimal voice-call client: one WebSocket to our own user server + one
 *  RTCPeerConnection (browser ↔ our server). Audio only. */

type CallStatus = 'idle' | 'ringing_out' | 'ringing_in' | 'connecting' | 'in_call';

interface RosterEntry {
  server: string;
  members: string[];
  muted: string[];
}

interface IncomingCall {
  call_id: string;
  from: string;
}

interface CallCtx {
  status: CallStatus;
  peers: string[];
  incoming: IncomingCall | null;
  muted: boolean;
  error: string | null;
  /** id of the voice channel currently joined, if any */
  voiceChannel: string | null;
  startCall: (peerFullId: string) => void;
  accept: () => void;
  reject: () => void;
  hangup: () => void;
  toggleMute: () => void;
  joinRoom: (channelServer: string, channelId: string) => void;
  leaveRoom: () => void;
}

const Ctx = createContext<CallCtx | null>(null);
// eslint-disable-next-line react-refresh/only-export-components
export const useCall = () => {
  const c = useContext(Ctx);
  if (!c) throw new Error('useCall outside CallProvider');
  return c;
};

function wsUrl() {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  return `${proto}://${location.host}/api/calls/ws`;
}

export function CallProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<CallStatus>('idle');
  const [peers, setPeers] = useState<string[]>([]);
  const [incoming, setIncoming] = useState<IncomingCall | null>(null);
  const [muted, setMuted] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [voiceChannel, setVoiceChannel] = useState<string | null>(null);

  const ws = useRef<WebSocket | null>(null);
  const pc = useRef<RTCPeerConnection | null>(null);
  const localStream = useRef<MediaStream | null>(null);
  const audioEl = useRef<HTMLAudioElement | null>(null);
  const callId = useRef<string | null>(null);
  const negotiated = useRef(false);

  const send = useCallback((msg: unknown) => {
    if (ws.current?.readyState === WebSocket.OPEN) ws.current.send(JSON.stringify(msg));
  }, []);

  const cleanup = useCallback(() => {
    pc.current?.close();
    pc.current = null;
    localStream.current?.getTracks().forEach((t) => t.stop());
    localStream.current = null;
    if (audioEl.current) audioEl.current.srcObject = null;
    callId.current = null;
    negotiated.current = false;
    setStatus('idle');
    setPeers([]);
    setMuted(false);
    setVoiceChannel(null);
  }, []);

  const negotiate = useCallback(async () => {
    if (negotiated.current || !callId.current) return;
    negotiated.current = true;
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      localStream.current = stream;
      const conn = new RTCPeerConnection();
      pc.current = conn;
      stream.getTracks().forEach((t) => conn.addTrack(t, stream));
      conn.ontrack = (e) => {
        if (audioEl.current) audioEl.current.srcObject = e.streams[0];
      };
      conn.onicecandidate = (e) => {
        if (e.candidate) {
          send({
            type: 'ice',
            call_id: callId.current,
            from: 'browser',
            to: 'server',
            candidate: JSON.stringify(e.candidate),
          });
        }
      };
      conn.onconnectionstatechange = () => {
        if (conn.connectionState === 'connected') setStatus('in_call');
      };
      const offer = await conn.createOffer();
      await conn.setLocalDescription(offer);
      send({ type: 'sdp', call_id: callId.current, from: 'browser', to: 'server', sdp: offer.sdp, answer: false });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'microphone unavailable');
      cleanup();
    }
  }, [send, cleanup]);

  const onMessage = useCallback(
    async (raw: string) => {
      let m: Record<string, unknown>;
      try {
        m = JSON.parse(raw);
      } catch {
        return;
      }
      // Learn our call id from the first frame that carries one.
      if (typeof m.call_id === 'string' && m.call_id && !callId.current && m.type !== 'incoming_call') {
        callId.current = m.call_id;
      }
      switch (m.type) {
        case 'incoming_call':
          setIncoming({ call_id: m.call_id as string, from: m.from as string });
          setStatus('ringing_in');
          break;
        case 'roster': {
          const parts = (m.participants as RosterEntry[]) ?? [];
          const members = parts.flatMap((p) => p.members);
          setPeers(members);
          if (callId.current && parts.length >= 1) {
            setStatus((s) => (s === 'in_call' ? s : 'connecting'));
            void negotiate();
          }
          break;
        }
        case 'sdp':
          if (m.answer && pc.current) {
            await pc.current.setRemoteDescription({ type: 'answer', sdp: m.sdp as string });
          }
          break;
        case 'ice':
          if (pc.current && typeof m.candidate === 'string') {
            try {
              await pc.current.addIceCandidate(JSON.parse(m.candidate));
            } catch {
              /* ignore */
            }
          }
          break;
        case 'call_ended':
          setIncoming(null);
          cleanup();
          break;
        case 'error':
          setError(m.message as string);
          break;
      }
    },
    [negotiate, cleanup],
  );

  useEffect(() => {
    let alive = true;
    let retry: ReturnType<typeof setTimeout>;
    const connect = () => {
      const sock = new WebSocket(wsUrl());
      ws.current = sock;
      sock.onmessage = (e) => void onMessage(e.data);
      sock.onclose = () => {
        if (alive) retry = setTimeout(connect, 2000);
      };
      sock.onerror = () => sock.close();
    };
    connect();
    return () => {
      alive = false;
      clearTimeout(retry);
      ws.current?.close();
    };
  }, [onMessage]);

  const startCall = useCallback(
    (peerFullId: string) => {
      setError(null);
      callId.current = null;
      negotiated.current = false;
      setStatus('ringing_out');
      send({ type: 'start_call', to: peerFullId });
      // the manager assigns the call id; onMessage picks it up from the first roster frame.
    },
    [send],
  );

  const accept = useCallback(() => {
    if (!incoming) return;
    callId.current = incoming.call_id;
    setIncoming(null);
    setStatus('connecting');
    send({ type: 'accept', call_id: incoming.call_id });
    void negotiate();
  }, [incoming, send, negotiate]);

  const reject = useCallback(() => {
    if (incoming) send({ type: 'reject', call_id: incoming.call_id });
    setIncoming(null);
    setStatus('idle');
  }, [incoming, send]);

  const hangup = useCallback(() => {
    if (callId.current) send({ type: 'hangup', call_id: callId.current });
    cleanup();
  }, [send, cleanup]);

  const toggleMute = useCallback(() => {
    const next = !muted;
    setMuted(next);
    localStream.current?.getAudioTracks().forEach((t) => (t.enabled = !next));
    if (callId.current) send({ type: 'mute', call_id: callId.current, muted: next });
  }, [muted, send]);

  const joinRoom = useCallback(
    (channelServer: string, channelId: string) => {
      setError(null);
      callId.current = null;
      negotiated.current = false;
      setVoiceChannel(channelId);
      setStatus('connecting');
      send({ type: 'join_room', channel_server: channelServer, channel_id: channelId });
    },
    [send],
  );

  const leaveRoom = useCallback(() => {
    send({ type: 'leave', call_id: '' });
    cleanup();
  }, [send, cleanup]);

  return (
    <Ctx.Provider
      value={{
        status, peers, incoming, muted, error, voiceChannel,
        startCall, accept, reject, hangup, toggleMute, joinRoom, leaveRoom,
      }}
    >
      {children}
      <audio ref={audioEl} autoPlay hidden />
    </Ctx.Provider>
  );
}
