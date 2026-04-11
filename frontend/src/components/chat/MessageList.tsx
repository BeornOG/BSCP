import { useEffect, useRef, useCallback, type FC, type MouseEvent } from 'react';
import MessageBubble from './MessageBubble';
import { Spinner } from '../ui';
import type { Message } from '../../types';

interface MessageListProps {
  messages: Message[];
  currentUser: string;
  isLoading: boolean;
  onImageClick: (src: string) => void;
}

const MessageList: FC<MessageListProps> = ({
  messages,
  currentUser,
  isLoading,
  onImageClick,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const shouldAutoScroll = useRef(true);
  const prevMessageCount = useRef(0);
  const profilePicCache = useRef<Record<string, string>>({});

  // Fetch profile pics for new senders
  useEffect(() => {
    const newSenders = messages
      .map((m) => m.sender)
      .filter(
        (sender, i, arr) =>
          arr.indexOf(sender) === i && !(sender in profilePicCache.current)
      );

    if (newSenders.length === 0) return;

    // Mark as pending to avoid duplicate fetches
    newSenders.forEach((s) => {
      profilePicCache.current[s] = '';
    });

    fetch('/api/users/batch', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ senders: newSenders }),
    })
      .then((res) => res.json())
      .then((data: Record<string, string>) => {
        Object.entries(data).forEach(([sender, pic]) => {
          profilePicCache.current[sender] = pic;
        });
      })
      .catch(() => {
        // Silently fail - avatars will use fallback
      });
  }, [messages]);

  // Track scroll position to decide auto-scroll
  const handleScroll = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    shouldAutoScroll.current = distanceFromBottom <= 60;
  }, []);

  // Auto-scroll on new messages
  useEffect(() => {
    if (messages.length > prevMessageCount.current || shouldAutoScroll.current) {
      bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
    prevMessageCount.current = messages.length;
  }, [messages]);

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
      {messages.map((msg) => {
        const isPending = msg.id.startsWith('pending-');
        const isFailed = msg.id.startsWith('failed-');

        return (
          <MessageBubble
            key={msg.id}
            message={msg}
            isOwn={msg.sender === currentUser}
            isPending={isPending}
            isFailed={isFailed}
            profilePic={profilePicCache.current[msg.sender] || undefined}
          />
        );
      })}
      <div ref={bottomRef} />
    </div>
  );
};

export default MessageList;
