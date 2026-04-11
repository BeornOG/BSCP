import { useState, useEffect, useRef, useCallback } from 'react';
import { marked } from 'marked';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface Message {
  id: string;
  sender: string;
  text: string;
  time: number;
  sender_profile_pic?: string;
}

interface Chat {
  id: string;
  display_name: string;
}

interface UserSettings {
  display_name: string;
  theme: string;
  accent_color: string;
  profile_pic: string;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ACCENT_COLORS = ['#7eafff', '#ff716c', '#e9caf0', '#4d3755', '#28a745'];

const THEMES: { value: string; label: string }[] = [
  { value: 'dark', label: 'Midnight Slate' },
  { value: 'light', label: 'Alabaster Muse' },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function getCookie(name: string): string {
  const v = document.cookie.match('(^|;)\\s*' + name + '\\s*=\\s*([^;]+)');
  return v ? v.pop()! : '';
}

/** Convert bare image / video URLs in plain text to markdown before rendering. */
function preprocessMarkdown(text: string): string {
  return text.replace(
    /(?<![("'])(https?:\/\/\S+\.(?:png|jpg|jpeg|gif|webp|svg))(?![)"'])/gi,
    '![]($1)',
  ).replace(
    /(?<![("'])(https?:\/\/\S+\.(?:mp4|webm|mov))(?![)"'])/gi,
    '<video controls src="$1" style="max-width:100%;border-radius:8px;margin-top:8px"></video>',
  );
}

/** Proxy external image src attributes through the backend. */
function proxyImages(html: string): string {
  return html.replace(/<img\s+([^>]*?)src="(https?:\/\/[^"]+)"([^>]*)>/g, (_m, pre, url, post) => {
    const proxied = `/media/proxy?url=${encodeURIComponent(url)}`;
    return `<img ${pre}src="${proxied}"${post}>`;
  });
}

function renderMarkdown(text: string): string {
  const preprocessed = preprocessMarkdown(text);
  const html = marked.parse(preprocessed, { async: false }) as string;
  return proxyImages(html);
}

function formatTime(unixSeconds: number): string {
  try {
    const d = new Date(unixSeconds * 1000);
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  } catch {
    return String(unixSeconds);
  }
}

function loadSettings(): UserSettings {
  try {
    const raw = localStorage.getItem('atelierSettings');
    if (raw) return JSON.parse(raw);
  } catch { /* ignore */ }
  return { display_name: '', theme: 'dark', accent_color: '#7eafff', profile_pic: '' };
}

function saveSettingsLocal(s: UserSettings) {
  localStorage.setItem('atelierSettings', JSON.stringify(s));
}

function applyTheme(theme: string) {
  document.body.classList.remove('theme-light', 'theme-dark');
  if (theme === 'light') document.body.classList.add('theme-light');
}

function applyAccent(color: string) {
  document.documentElement.style.setProperty('--dynamic-primary', color);
}

// ---------------------------------------------------------------------------
// Sub-components (defined in the same file)
// ---------------------------------------------------------------------------

function ImageModal({ src, onClose }: { src: string; onClose: () => void }) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/80"
      onClick={onClose}
    >
      <button
        className="absolute top-4 right-4 text-white"
        onClick={onClose}
      >
        <span className="material-symbols-outlined text-3xl">close</span>
      </button>
      <img
        src={src}
        className="max-w-[90vw] max-h-[90vh] rounded-lg object-contain"
        onClick={(e) => e.stopPropagation()}
        alt=""
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export default function ChatPage() {
  // ---- auth state ----
  const [currentUser, setCurrentUser] = useState('');
  const [defaultDisplayName, setDefaultDisplayName] = useState('');

  // ---- view toggle ----
  const [view, setView] = useState<'chat' | 'settings'>('chat');

  // ---- chats ----
  const [chats, setChats] = useState<Chat[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [activeChatName, setActiveChatName] = useState('');

  // ---- messages ----
  const [messages, setMessages] = useState<Message[]>([]);
  const [messageInput, setMessageInput] = useState('');
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [allOlderLoaded, setAllOlderLoaded] = useState(false);

  // ---- profile pics cache ----
  const profilePicCache = useRef<Record<string, string>>({});

  // ---- settings ----
  const [settings, setSettings] = useState<UserSettings>(loadSettings);

  // ---- pending (optimistic) messages ----
  const [pendingMessages, setPendingMessages] = useState<Message[]>([]);

  // ---- image modal ----
  const [modalImage, setModalImage] = useState<string | null>(null);

  // ---- new chat ----
  const [showNewChat, setShowNewChat] = useState(false);
  const [newChatReceiver, setNewChatReceiver] = useState('');

  // ---- refs ----
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const prevScrollHeight = useRef(0);
  const shouldAutoScroll = useRef(true);

  // ---- fetch helpers ----
  const fetchJson = useCallback(async (url: string, opts?: RequestInit) => {
    const res = await fetch(url, {
      credentials: 'include',
      headers: { 'X-CSRFToken': getCookie('csrftoken'), ...opts?.headers },
      ...opts,
    });
    if (res.status === 401) {
      window.location.href = '/login';
      throw new Error('Unauthorized');
    }
    return res;
  }, []);

  // -----------------------------------------------------------------------
  // Initial load: user profile
  // -----------------------------------------------------------------------
  useEffect(() => {
    (async () => {
      try {
        const res = await fetchJson('/api/userprofile/');
        const data = await res.json();
        setCurrentUser(data.full_id ?? `${data.username}@${data.domain}` ?? '');
        setDefaultDisplayName(data.display_name ?? '');
        const s: UserSettings = {
          display_name: data.display_name ?? '',
          theme: data.theme ?? 'dark',
          accent_color: data.accent_color ?? '#7eafff',
          profile_pic: data.profile_pic ?? '',
        };
        setSettings(s);
        saveSettingsLocal(s);
        applyTheme(s.theme);
        applyAccent(s.accent_color);
      } catch {
        // fetchJson already redirects on 401
      }
    })();
  }, [fetchJson]);

  // -----------------------------------------------------------------------
  // Fetch chats
  // -----------------------------------------------------------------------
  const fetchChats = useCallback(async () => {
    try {
      const res = await fetchJson('/api/chats');
      const data: Chat[] = await res.json();
      setChats(data);
    } catch { /* ignore */ }
  }, [fetchJson]);

  useEffect(() => {
    fetchChats();
  }, [fetchChats]);

  // -----------------------------------------------------------------------
  // Fetch messages for active chat
  // -----------------------------------------------------------------------
  const fetchMessages = useCallback(async (chatId: string, isPolling = false) => {
    try {
      const res = await fetchJson(`/api/messages/${encodeURIComponent(chatId)}`);
      let data: Message[] = await res.json();
      if (!Array.isArray(data)) data = [];
      data.sort((a, b) => a.time - b.time);

      setMessages((prev) => {
        // If polling, only update if the data actually changed
        if (isPolling && prev.length === data.length && prev.length > 0 && prev[prev.length - 1].id === data[data.length - 1].id) {
          return prev; // No change, skip re-render
        }
        if (!isPolling || prev.length === 0) {
          shouldAutoScroll.current = true;
        } else if (data.length > prev.length) {
          // New messages arrived during poll — auto-scroll if near bottom
          shouldAutoScroll.current = true;
        }
        return data;
      });

      // Clear pending messages that now appear in the fetched data
      setPendingMessages((pending) =>
        pending.filter((p) => !data.some((m) => m.sender === p.sender && m.text === p.text))
      );

      // resolve profile pics
      const senders = [...new Set(data.map((m) => m.sender))].filter(
        (s) => !profilePicCache.current[s],
      );
      if (senders.length) {
        try {
          const batchRes = await fetchJson('/api/userprofile/batch', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ senders }),
          });
          const batchData = await batchRes.json();
          profilePicCache.current = { ...profilePicCache.current, ...batchData };
        } catch { /* ignore */ }
      }
    } catch { /* ignore */ }
  }, [fetchJson]);

  const fetchOlderMessages = useCallback(async () => {
    if (!activeChatId || loadingOlder || allOlderLoaded) return;
    const oldest = messages[0];
    if (!oldest) return;
    setLoadingOlder(true);
    try {
      const res = await fetchJson(`/api/messages/${encodeURIComponent(activeChatId)}?before=${oldest.time}`);
      let data: Message[] = await res.json();
      if (!Array.isArray(data)) data = [];
      data.sort((a, b) => a.time - b.time);
      if (data.length === 0) {
        setAllOlderLoaded(true);
      } else {
        prevScrollHeight.current = messagesContainerRef.current?.scrollHeight ?? 0;
        setMessages((prev) => {
          const existingIds = new Set(prev.map((m) => m.id));
          const newMsgs = data.filter((m) => !existingIds.has(m.id));
          return newMsgs.length ? [...newMsgs, ...prev] : prev;
        });
      }
    } catch { /* ignore */ }
    setLoadingOlder(false);
  }, [activeChatId, loadingOlder, allOlderLoaded, messages, fetchJson]);

  // When active chat changes, load messages
  useEffect(() => {
    if (activeChatId) {
      setMessages([]);
      setPendingMessages([]);
      setAllOlderLoaded(false);
      fetchMessages(activeChatId);
    }
  }, [activeChatId, fetchMessages]);

  // -----------------------------------------------------------------------
  // Polling (1 s) – new messages + chat list refresh
  // -----------------------------------------------------------------------
  useEffect(() => {
    if (view !== 'chat') return;
    const interval = setInterval(() => {
      fetchChats();
      if (activeChatId) {
        fetchMessages(activeChatId, true);
      }
    }, 1000);
    return () => clearInterval(interval);
  }, [view, activeChatId, fetchChats, fetchMessages]);

  // -----------------------------------------------------------------------
  // Auto-scroll to bottom on new messages
  // -----------------------------------------------------------------------
  useEffect(() => {
    if (shouldAutoScroll.current) {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [messages]);

  // Maintain scroll position when prepending older messages
  useEffect(() => {
    if (prevScrollHeight.current && messagesContainerRef.current) {
      const newHeight = messagesContainerRef.current.scrollHeight;
      messagesContainerRef.current.scrollTop = newHeight - prevScrollHeight.current;
      prevScrollHeight.current = 0;
    }
  }, [messages]);

  // -----------------------------------------------------------------------
  // Infinite scroll – load older
  // -----------------------------------------------------------------------
  const handleScroll = useCallback(() => {
    const el = messagesContainerRef.current;
    if (!el) return;
    shouldAutoScroll.current = el.scrollHeight - el.scrollTop - el.clientHeight < 60;
    if (el.scrollTop < 80) {
      fetchOlderMessages();
    }
  }, [fetchOlderMessages]);

  // -----------------------------------------------------------------------
  // Send message
  // -----------------------------------------------------------------------
  const sendMessage = useCallback(async () => {
    const text = messageInput.trim();
    if (!text || !activeChatName) return;

    // Clear input immediately
    setMessageInput('');

    // Add optimistic pending message
    const pendingId = `pending-${Date.now()}`;
    const pendingMsg: Message = {
      id: pendingId,
      sender: currentUser,
      text,
      time: Date.now() / 1000,
    };
    setPendingMessages((prev) => [...prev, pendingMsg]);
    shouldAutoScroll.current = true;

    try {
      await fetchJson('/api/sendmessage', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ receiver: activeChatName, messageText: text }),
      });
      // Remove pending message and fetch latest
      setPendingMessages((prev) => prev.filter((m) => m.id !== pendingId));
      if (activeChatId) {
        fetchMessages(activeChatId);
      }
    } catch {
      // Mark pending message as failed
      setPendingMessages((prev) =>
        prev.map((m) => (m.id === pendingId ? { ...m, id: `failed-${pendingId}` } : m)),
      );
    }
  }, [messageInput, activeChatName, activeChatId, currentUser, fetchJson, fetchMessages]);

  // -----------------------------------------------------------------------
  // File upload
  // -----------------------------------------------------------------------
  const handleFileUpload = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const fd = new FormData();
    fd.append('file', file);
    try {
      const res = await fetchJson('/api/upload', { method: 'POST', body: fd });
      const data = await res.json();
      if (data.markdown) {
        setMessageInput((prev) => prev + (prev ? '\n' : '') + data.markdown);
      }
    } catch { /* ignore */ }
    e.target.value = '';
  }, [fetchJson]);

  // -----------------------------------------------------------------------
  // Image click handler (delegated)
  // -----------------------------------------------------------------------
  const handleMessageAreaClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.tagName === 'IMG' && target.closest('.msg-content')) {
      setModalImage((target as HTMLImageElement).src);
    }
  }, []);

  // -----------------------------------------------------------------------
  // Settings handlers
  // -----------------------------------------------------------------------
  const updateSetting = <K extends keyof UserSettings>(key: K, value: UserSettings[K]) => {
    setSettings((prev) => {
      const next = { ...prev, [key]: value };
      saveSettingsLocal(next);
      if (key === 'theme') applyTheme(value as string);
      if (key === 'accent_color') applyAccent(value as string);
      return next;
    });
  };

  const saveSettings = async () => {
    try {
      await fetchJson('/api/userprofile/', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          display_name: settings.display_name,
          theme: settings.theme,
          accent_color: settings.accent_color,
        }),
      });
    } catch { /* ignore */ }
  };

  const handleProfilePicUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const fd = new FormData();
    fd.append('file', file);
    try {
      const res = await fetchJson('/api/userprofile/picture', { method: 'POST', body: fd });
      const data = await res.json();
      if (data.profile_pic) updateSetting('profile_pic', data.profile_pic);
    } catch { /* ignore */ }
    e.target.value = '';
  };

  const deleteProfilePic = async () => {
    try {
      await fetchJson('/api/userprofile/picture', { method: 'DELETE' });
      updateSetting('profile_pic', '');
    } catch { /* ignore */ }
  };

  const handleLogout = async () => {
    await fetch('/api/auth/logout', { method: 'POST' });
    window.location.href = '/login';
  };

  // -----------------------------------------------------------------------
  // New chat
  // -----------------------------------------------------------------------
  const startNewChat = async () => {
    const receiver = newChatReceiver.trim();
    if (!receiver) return;
    await fetchJson('/api/sendmessage', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ receiver, messageText: 'Hello!' }),
    });
    setNewChatReceiver('');
    setShowNewChat(false);
    await fetchChats();
  };

  // -----------------------------------------------------------------------
  // Select a chat
  // -----------------------------------------------------------------------
  const selectChat = (chat: Chat) => {
    setActiveChatId(chat.id);
    setActiveChatName(chat.display_name);
  };

  // -----------------------------------------------------------------------
  // Render
  // -----------------------------------------------------------------------
  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#0c0f10] text-[#f8f9fc]">
      {/* ---- Left nav rail ---- */}
      <nav className="flex flex-col items-center justify-between w-20 min-w-[5rem] py-6 bg-[#0c0f10] border-r border-[#222629]">
        <div className="flex flex-col items-center gap-6">
          {/* Logo */}
          <span className="text-2xl font-bold" style={{ color: 'var(--dynamic-primary)' }}>
            A.
          </span>

          {/* Chat button */}
          <button
            onClick={() => setView('chat')}
            className={`p-2 rounded-xl transition-colors ${view === 'chat' ? 'bg-[#222629]' : 'hover:bg-[#1c2023]'}`}
            title="Messages"
          >
            <span className="material-symbols-outlined text-2xl">chat</span>
          </button>
        </div>

        <div className="flex flex-col items-center gap-4">
          {/* Settings button */}
          <button
            onClick={() => setView('settings')}
            className={`p-2 rounded-xl transition-colors ${view === 'settings' ? 'bg-[#222629]' : 'hover:bg-[#1c2023]'}`}
            title="Settings"
          >
            <span className="material-symbols-outlined text-2xl">settings</span>
          </button>

          {/* User avatar */}
          <div className="w-9 h-9 rounded-full overflow-hidden bg-[#222629] flex items-center justify-center">
            {settings.profile_pic ? (
              <img src={settings.profile_pic} alt="" className="w-full h-full object-cover" />
            ) : (
              <span className="material-symbols-outlined text-lg">person</span>
            )}
          </div>
        </div>
      </nav>

      {/* ---- Main content area ---- */}
      <div className="flex flex-1 overflow-hidden">
        {view === 'chat' ? (
          <>
            {/* ---- Chat sidebar ---- */}
            <aside className="flex flex-col w-80 min-w-[20rem] bg-[#0c0f10] border-r border-[#222629]">
              <div className="flex items-center justify-between px-5 py-4">
                <h2 className="text-lg font-semibold">Messages</h2>
                <button
                  onClick={() => setShowNewChat(true)}
                  className="p-1.5 rounded-lg hover:bg-[#1c2023] transition-colors"
                  title="New Chat"
                >
                  <span className="material-symbols-outlined">add_circle</span>
                </button>
              </div>

              {/* New chat input */}
              {showNewChat && (
                <div className="px-4 pb-3 flex gap-2">
                  <input
                    type="text"
                    placeholder="Username..."
                    value={newChatReceiver}
                    onChange={(e) => setNewChatReceiver(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && startNewChat()}
                    className="flex-1 px-3 py-1.5 rounded-lg bg-[#1c2023] border border-[#222629] text-sm text-[#f8f9fc] placeholder-gray-500 focus:outline-none focus:border-[var(--dynamic-primary)]"
                    autoFocus
                  />
                  <button
                    onClick={startNewChat}
                    className="px-3 py-1.5 rounded-lg text-sm font-medium text-white"
                    style={{ backgroundColor: 'var(--dynamic-primary)' }}
                  >
                    Go
                  </button>
                  <button
                    onClick={() => { setShowNewChat(false); setNewChatReceiver(''); }}
                    className="p-1.5 rounded-lg hover:bg-[#1c2023]"
                  >
                    <span className="material-symbols-outlined text-sm">close</span>
                  </button>
                </div>
              )}

              {/* Chat list */}
              <div className="flex-1 overflow-y-auto px-2">
                {chats.map((chat) => (
                  <button
                    key={chat.id}
                    onClick={() => selectChat(chat)}
                    className={`w-full flex items-center gap-3 px-3 py-3 rounded-xl mb-1 text-left transition-colors ${
                      activeChatId === chat.id ? 'bg-[#1c2023]' : 'hover:bg-[#1c2023]/50'
                    }`}
                  >
                    <div className="w-10 h-10 rounded-full bg-[#222629] flex items-center justify-center shrink-0">
                      <span className="material-symbols-outlined text-lg">person</span>
                    </div>
                    <div className="overflow-hidden">
                      <p className="text-sm font-medium truncate">{chat.display_name}</p>
                    </div>
                  </button>
                ))}
              </div>
            </aside>

            {/* ---- Chat main area ---- */}
            <main className="flex flex-col flex-1 overflow-hidden">
              {activeChatId ? (
                <>
                  {/* Chat header */}
                  <header className="flex items-center gap-3 px-6 py-4 border-b border-[#222629] bg-[#0c0f10]">
                    <div className="w-9 h-9 rounded-full bg-[#222629] flex items-center justify-center">
                      <span className="material-symbols-outlined text-lg">person</span>
                    </div>
                    <h3 className="text-base font-semibold">{activeChatName}</h3>
                  </header>

                  {/* Messages */}
                  <div
                    ref={messagesContainerRef}
                    className="flex-1 overflow-y-auto px-6 py-4 space-y-4"
                    onScroll={handleScroll}
                    onClick={handleMessageAreaClick}
                  >
                    {loadingOlder && (
                      <p className="text-center text-xs text-gray-500 py-2">Loading older messages...</p>
                    )}
                    {[...messages, ...pendingMessages].map((msg) => {
                      const isMe = msg.sender === currentUser;
                      const isPending = typeof msg.id === 'string' && msg.id.startsWith('pending-');
                      const isFailed = typeof msg.id === 'string' && msg.id.startsWith('failed-');
                      const pic = msg.sender_profile_pic || profilePicCache.current[msg.sender];
                      return (
                        <div
                          key={msg.id}
                          className={`flex gap-3 ${isMe ? 'flex-row-reverse' : ''} ${isPending ? 'opacity-60' : ''}`}
                        >
                          {/* Avatar */}
                          <div className="w-8 h-8 rounded-full overflow-hidden bg-[#222629] flex items-center justify-center shrink-0 mt-1">
                            {pic ? (
                              <img src={pic} alt="" className="w-full h-full object-cover" />
                            ) : (
                              <span className="material-symbols-outlined text-sm">person</span>
                            )}
                          </div>

                          {/* Bubble */}
                          <div className={`max-w-[65%] ${isMe ? 'items-end' : 'items-start'}`}>
                            <div className="flex items-baseline gap-2 mb-1">
                              <span className={`text-xs font-medium ${isMe ? 'text-right w-full block' : ''}`}>
                                {msg.sender}
                              </span>
                              <span className="text-[10px] text-gray-500 whitespace-nowrap">
                                {isPending ? 'Sending...' : isFailed ? 'Failed to send' : formatTime(msg.time)}
                              </span>
                            </div>
                            <div
                              className={`msg-content rounded-2xl px-4 py-2.5 text-sm leading-relaxed ${
                                isMe
                                  ? 'rounded-tr-sm text-white'
                                  : 'bg-[#1c2023] rounded-tl-sm'
                              }`}
                              style={isMe ? { backgroundColor: 'var(--dynamic-primary)' } : undefined}
                              dangerouslySetInnerHTML={{ __html: renderMarkdown(msg.text) }}
                            />
                          </div>
                        </div>
                      );
                    })}
                    <div ref={messagesEndRef} />
                  </div>

                  {/* Message input footer */}
                  <footer className="px-6 py-4 border-t border-[#222629] bg-[#0c0f10]">
                    <div className="flex items-center gap-3 bg-[#1c2023] rounded-2xl px-4 py-2">
                      <button
                        onClick={() => fileInputRef.current?.click()}
                        className="p-1 rounded-lg hover:bg-[#222629] transition-colors"
                        title="Upload file"
                      >
                        <span className="material-symbols-outlined text-xl">add_circle</span>
                      </button>
                      <input
                        ref={fileInputRef}
                        type="file"
                        className="hidden"
                        onChange={handleFileUpload}
                      />
                      <input
                        type="text"
                        value={messageInput}
                        onChange={(e) => setMessageInput(e.target.value)}
                        onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && (e.preventDefault(), sendMessage())}
                        placeholder="Type a message..."
                        className="flex-1 bg-transparent text-sm text-[#f8f9fc] placeholder-gray-500 focus:outline-none"
                      />
                      <button
                        onClick={sendMessage}
                        className="p-2 rounded-xl transition-colors"
                        style={{ backgroundColor: 'var(--dynamic-primary)' }}
                        title="Send"
                      >
                        <span className="material-symbols-outlined text-xl text-white">send</span>
                      </button>
                    </div>
                  </footer>
                </>
              ) : (
                <div className="flex-1 flex items-center justify-center text-gray-500 text-sm">
                  Select a conversation to start messaging
                </div>
              )}
            </main>
          </>
        ) : (
          /* ---- Settings view ---- */
          <main className="flex-1 overflow-y-auto px-8 py-10 max-w-2xl mx-auto">
            <h2 className="text-2xl font-bold mb-8">Settings</h2>

            {/* Profile section */}
            <section className="mb-10">
              <h3 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4">Profile</h3>
              <div className="flex items-center gap-5 mb-5">
                <div className="relative group">
                  <div className="w-20 h-20 rounded-full overflow-hidden bg-[#222629] flex items-center justify-center">
                    {settings.profile_pic ? (
                      <img src={settings.profile_pic} alt="" className="w-full h-full object-cover" />
                    ) : (
                      <span className="material-symbols-outlined text-3xl">person</span>
                    )}
                  </div>
                  <div className="absolute inset-0 rounded-full bg-black/50 opacity-0 group-hover:opacity-100 flex items-center justify-center transition-opacity">
                    <label className="cursor-pointer">
                      <span className="material-symbols-outlined text-white">add_circle</span>
                      <input type="file" accept="image/*" className="hidden" onChange={handleProfilePicUpload} />
                    </label>
                  </div>
                </div>
                <div>
                  <p className="font-medium">{settings.display_name || defaultDisplayName || currentUser}</p>
                  <p className="text-sm text-gray-500">{currentUser}</p>
                  {settings.profile_pic && (
                    <button
                      onClick={deleteProfilePic}
                      className="text-xs text-red-400 hover:text-red-300 mt-1"
                    >
                      Remove picture
                    </button>
                  )}
                </div>
              </div>

              <label className="block mb-1 text-sm text-gray-400">Display Name</label>
              <input
                type="text"
                value={settings.display_name}
                onChange={(e) => updateSetting('display_name', e.target.value)}
                className="w-full px-4 py-2.5 rounded-xl bg-[#1c2023] border border-[#222629] text-sm text-[#f8f9fc] focus:outline-none focus:border-[var(--dynamic-primary)]"
              />
            </section>

            {/* Theme section */}
            <section className="mb-10">
              <h3 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4">Theme</h3>
              <div className="flex gap-3">
                {THEMES.map((t) => (
                  <button
                    key={t.value}
                    onClick={() => updateSetting('theme', t.value)}
                    className={`flex-1 px-4 py-3 rounded-xl text-sm font-medium border transition-colors ${
                      settings.theme === t.value
                        ? 'border-[var(--dynamic-primary)] bg-[#1c2023]'
                        : 'border-[#222629] bg-[#1c2023] hover:border-[#333]'
                    }`}
                    style={settings.theme === t.value ? { borderColor: 'var(--dynamic-primary)' } : undefined}
                  >
                    {t.label}
                  </button>
                ))}
              </div>
            </section>

            {/* Accent color section */}
            <section className="mb-10">
              <h3 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4">Accent Color</h3>
              <div className="flex gap-3">
                {ACCENT_COLORS.map((color) => (
                  <button
                    key={color}
                    onClick={() => updateSetting('accent_color', color)}
                    className={`w-10 h-10 rounded-full transition-transform ${
                      settings.accent_color === color ? 'scale-110 ring-2 ring-white ring-offset-2 ring-offset-[#0c0f10]' : 'hover:scale-105'
                    }`}
                    style={{ backgroundColor: color }}
                    title={color}
                  />
                ))}
              </div>
            </section>

            {/* Action buttons */}
            <div className="flex gap-3">
              <button
                onClick={saveSettings}
                className="px-6 py-2.5 rounded-xl text-sm font-medium text-white transition-colors"
                style={{ backgroundColor: 'var(--dynamic-primary)' }}
              >
                Save Settings
              </button>
              <button
                onClick={handleLogout}
                className="px-6 py-2.5 rounded-xl text-sm font-medium bg-[#1c2023] border border-[#222629] hover:bg-[#222629] transition-colors"
              >
                Logout
              </button>
            </div>
          </main>
        )}
      </div>

      {/* ---- Image modal ---- */}
      {modalImage && <ImageModal src={modalImage} onClose={() => setModalImage(null)} />}
    </div>
  );
}
