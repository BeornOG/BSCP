import { useState, useEffect, useRef, useCallback, useMemo, type FC, type MouseEvent } from 'react';
import MessageBubble from './MessageBubble';
import { Spinner } from '../ui';
import type { Message } from '../../types';

interface MessageListProps {
  chatId: string | null;
  messages: Message[];
  currentUser: string;
  isLoading: boolean;
  onImageClick: (src: string) => void;
}

const MessageList: FC<MessageListProps> = ({
  chatId,
  messages,
  currentUser,
  isLoading,
  onImageClick,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const unreadMarkerRef = useRef<HTMLDivElement>(null);
  const shouldAutoScroll = useRef(true);
  const prevMessageCount = useRef(0);
  const [profilePics, setProfilePics] = useState<Record<string, string>>({});
  const [displayNames, setDisplayNames] = useState<Record<string, string>>({});
  const fetchedSenders = useRef<Set<string>>(new Set());
  const [hasDoneInitialScroll, setHasDoneInitialScroll] = useState(false);
  const [hideUnreadBoundary, setHideUnreadBoundary] = useState(false);

  const firstUnreadIndex = useMemo(
    () => messages.findIndex((m) => !m.is_read && m.sender !== currentUser),
    [messages, currentUser]
  );
  const hasInitialUnread = firstUnreadIndex !== -1;
  const showUnreadBoundary = hasInitialUnread && !hideUnreadBoundary;


  // Fetch profile pics for new senders
  useEffect(() => {
    const newSenders = messages
      .map((m) => m.sender)
      .filter(
        (sender, i, arr) =>
          arr.indexOf(sender) === i && !fetchedSenders.current.has(sender)
      );

    if (newSenders.length === 0) return;

    // Mark as pending to avoid duplicate fetches
    newSenders.forEach((s) => fetchedSenders.current.add(s));

    newSenders.forEach((sender) => {
      fetch(`/api/users/${encodeURIComponent(sender)}`)
        .then((res) => res.ok ? res.json() : null)
        .then((data) => {
          if (data?.profile_pic) {
            setProfilePics((prev) => ({ ...prev, [sender]: data.profile_pic }));
          }
          if (data?.display_name) {
            setDisplayNames((prev) => ({ ...prev, [sender]: data.display_name }));
          }
        })
        .catch(() => {});
    });
  }, [messages]);

  // Reset auto-scroll state when switching conversations
  useEffect(() => {
    shouldAutoScroll.current = true;
    prevMessageCount.current = messages.length;
    setHasDoneInitialScroll(false);
    setHideUnreadBoundary(false);
  }, [chatId, messages.length]);

  // Track scroll position to decide auto-scroll
  const handleScroll = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    const atBottom = distanceFromBottom <= 60;
    shouldAutoScroll.current = atBottom;
  }, []);

  useEffect(() => {
    if (!chatId || hasDoneInitialScroll || messages.length === 0) return;

    if (firstUnreadIndex !== -1) {
      unreadMarkerRef.current?.scrollIntoView({ behavior: 'smooth', block: 'center' });

      // Auto-hide boundary after 5 seconds
      const timeout = setTimeout(() => {
        setHideUnreadBoundary(true);
      }, 5000);

      return () => clearTimeout(timeout);
    } else {
      bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
    }

    setHasDoneInitialScroll(true);
  }, [chatId, firstUnreadIndex, hasDoneInitialScroll, messages.length]);

  // Auto-scroll on new messages (but not until initial scroll is done)
  useEffect(() => {
    if (!hasDoneInitialScroll) return;

    if (messages.length > prevMessageCount.current || shouldAutoScroll.current) {
      bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
    prevMessageCount.current = messages.length;
  }, [messages, showUnreadBoundary, hasDoneInitialScroll]);

  // Click delegation for images
  const handleClick = (e: MouseEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (target.tagName === 'IMG' && target.closest('.msg-content')) {
      const src = (target as HTMLImageElement).src;
      if (src) onImageClick(src);
    }
  };

  if (isLoading && messages.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <Spinner size="md" />
      </div>
    );
  }

  if (!isLoading && messages.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <p className="text-sm text-[#71747a]">No messages yet</p>
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      onScroll={handleScroll}
      onClick={handleClick}
      className="flex flex-1 flex-col gap-4 overflow-y-auto px-6 py-4"
    >
      {messages.map((msg, index) => {
        const isPending = msg.id.startsWith('pending-');
        const isFailed = msg.id.startsWith('failed-');
        const isFirstUnread = index === firstUnreadIndex;

        return (
          <div key={msg.id}>
            {isFirstUnread && showUnreadBoundary && (
              <div
                ref={unreadMarkerRef}
                className="mb-4 flex items-center gap-3 text-xs uppercase tracking-[0.2em] text-red-300"
              >
                <div className="h-px flex-1 bg-red-500" />
                <span className="px-2 py-1 rounded-full bg-[#2a0d12] text-red-200">
                  Unread messages
                </span>
                <div className="h-px flex-1 bg-red-500" />
              </div>
            )}
            <MessageBubble
              message={msg}
              isOwn={msg.sender === currentUser}
              isPending={isPending}
              isFailed={isFailed}
              profilePic={profilePics[msg.sender] || undefined}
              displayName={displayNames[msg.sender]}
            />
          </div>
        );
      })}
      <div ref={bottomRef} />
    </div>
  );
};

export default MessageList;
