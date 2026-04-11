import type { FC } from 'react';
import { marked } from 'marked';
import { Avatar } from '../ui';
import type { Message } from '../../types';

interface MessageBubbleProps {
  message: Message;
  isOwn: boolean;
  isPending: boolean;
  isFailed: boolean;
  profilePic?: string;
}

/** Convert bare image/video URLs into markdown/HTML */
function preprocessMarkdown(text: string): string {
  // Convert bare image URLs to markdown images
  text = text.replace(
    /(?<!!)\b(https?:\/\/\S+\.(?:png|jpg|jpeg|gif|webp|svg))(?:\s|$)/gi,
    '![]($1) '
  );
  // Convert bare video URLs to <video> tags
  text = text.replace(
    /\b(https?:\/\/\S+\.(?:mp4|webm|ogg))(?:\s|$)/gi,
    '<video controls src="$1" class="max-w-full rounded-lg"></video> '
  );
  return text;
}

/** Rewrite external img src to proxy endpoint */
function proxyImages(html: string): string {
  return html.replace(
    /<img([^>]*?)src="(https?:\/\/[^"]+)"/g,
    (_match, attrs, url) =>
      `<img${attrs}src="/media/proxy?url=${encodeURIComponent(url)}"`
  );
}

/** Full render pipeline: preprocess -> marked -> proxy images */
export function renderMarkdown(text: string): string {
  const preprocessed = preprocessMarkdown(text);
  const html = marked.parse(preprocessed, { async: false }) as string;
  return proxyImages(html);
}

const MessageBubble: FC<MessageBubbleProps> = ({
  message,
  isOwn,
  isPending,
  isFailed,
  profilePic,
}) => {
  const renderedHtml = renderMarkdown(message.text);

  const formatTime = (ts: number): string => {
    return new Date(ts * 1000).toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  const statusText = isPending
    ? 'Sending...'
    : isFailed
      ? 'Failed'
      : formatTime(message.timestamp);

  return (
    <div
      className={`flex gap-2.5 ${isOwn ? 'flex-row-reverse' : 'flex-row'} ${
        isPending ? 'opacity-60' : ''
      }`}
    >
      <Avatar
        src={profilePic}
        size="sm"
      />

      <div className={`flex max-w-[70%] flex-col ${isOwn ? 'items-end' : 'items-start'}`}>
        <span className="mb-1 text-xs text-[#71747a]">{message.sender}</span>

        <div
          className={`msg-content rounded-2xl px-4 py-2.5 text-sm leading-relaxed ${
            isOwn
              ? 'rounded-tr-sm bg-[var(--accent)] text-white'
              : 'rounded-tl-sm bg-[#1a1d21] text-[#e8eaed]'
          }`}
          dangerouslySetInnerHTML={{ __html: renderedHtml }}
        />

        <span
          className={`mt-1 text-xs ${
            isFailed ? 'text-red-400' : 'text-[#71747a]'
          }`}
        >
          {statusText}
        </span>
      </div>
    </div>
  );
};

export default MessageBubble;
