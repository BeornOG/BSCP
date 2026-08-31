import type { FC } from 'react';
import { marked } from 'marked';
import { Avatar } from '../ui';
import type { Message } from '../../types';

// Configure marked to allow HTML
marked.setOptions({
  breaks: true,
});

interface MessageBubbleProps {
  message: Message;
  isOwn: boolean;
  isPending: boolean;
  isFailed: boolean;
  profilePic?: string;
  displayName?: string;
  onAvatarClick?: (sender: string) => void;
  onDelete?: (messageId: string) => void;
}

/** Convert bare image/video URLs into markdown/HTML */
function preprocessMarkdown(text: string): string {
  // Convert bare image URLs to markdown images
  text = text.replace(
    /(?<!!)\b(https?:\/\/\S+\.(?:png|jpg|jpeg|gif|webp|svg))(?:\s|$)/gi,
    '![]($1) '
  );

  // Convert YouTube URLs to iframe embeds
  text = text.replace(
    /(?:https?:\/\/)?(?:www\.)?(?:youtube\.com\/watch\?v=|youtu\.be\/)([a-zA-Z0-9_-]{11})/gi,
    '<iframe width="100%" height="315" src="https://www.youtube.com/embed/$1" frameborder="0" allowfullscreen></iframe> '
  );

  // Convert Vimeo URLs to iframe embeds
  text = text.replace(
    /(?:https?:\/\/)?(?:www\.)?vimeo\.com\/(\d+)/gi,
    '<iframe src="https://player.vimeo.com/video/$1" width="100%" height="315" frameborder="0" allowfullscreen></iframe> '
  );

  // Convert bare video file URLs to <video> tags (local paths, relative, or full URLs)
  text = text.replace(
    /\b((?:https?:)?\/\/[^\s]+\.(?:mp4|webm|ogg|mkv|avi|mov|flv|wmv)|\/[^\s]+\.(?:mp4|webm|ogg|mkv|avi|mov|flv|wmv))(?:\s|$)/gi,
    '<video controls style="max-width: 100%; border-radius: 0.5rem; margin: 0.25rem 0;"><source src="$1"></video> '
  );

  // Convert bare audio file URLs to <audio> tags
  text = text.replace(
    /\b((?:https?:)?\/\/[^\s]+\.(?:mp3|wav|flac|aac|m4a)|\/[^\s]+\.(?:mp3|wav|flac|aac|m4a))(?:\s|$)/gi,
    '<audio controls style="max-width: 100%; margin: 0.25rem 0;"><source src="$1"></audio> '
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

/**
 * Turn plain links to uploaded files (`.../uploads/<name>`) into a compact
 * download card instead of a bare hyperlink, so non-media attachments
 * (PDFs, archives, docs, …) still look like attachments. See issues #15/#17.
 */
function decorateFileLinks(html: string): string {
  return html.replace(
    /<a href="([^"]*\/uploads\/([^"]+))">([^<]*)<\/a>/g,
    (_m, href: string, stored: string, label: string) => {
      const name = label && label !== href ? label : decodeURIComponent(stored.replace(/^[0-9a-f-]{36}_/i, ''));
      return (
        `<a href="${href}" target="_blank" rel="noopener noreferrer" download ` +
        `style="display:inline-flex;align-items:center;gap:0.5rem;max-width:100%;` +
        `padding:0.5rem 0.75rem;margin:0.25rem 0;border-radius:0.5rem;` +
        `background:rgba(255,255,255,0.06);text-decoration:none;">` +
        `<span class="material-symbols-outlined" style="font-size:20px;flex-shrink:0;">attach_file</span>` +
        `<span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${name}</span>` +
        `</a>`
      );
    }
  );
}

/** Full render pipeline: preprocess -> marked -> proxy images -> file cards */
export function renderMarkdown(text: string): string {
  const preprocessed = preprocessMarkdown(text);
  const html = marked.parse(preprocessed, { async: false }) as string;
  return decorateFileLinks(proxyImages(html));
}

const MessageBubble: FC<MessageBubbleProps> = ({
  message,
  isOwn,
  isPending,
  isFailed,
  profilePic,
  displayName,
  onAvatarClick,
  onDelete,
}) => {
  const call = (message.kind === 'call_invite' || message.kind === 'call_end') ? message : null;
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

  const senderName = displayName || message.sender.split('@')[0];

  if (call) {
    const outcome = (call.metadata as Record<string, unknown> | undefined)?.outcome as string | undefined;
    const label =
      call.kind === 'call_invite'
        ? 'Call started'
        : outcome === 'rejected'
          ? 'Call declined'
          : outcome === 'missed'
            ? 'Missed call'
            : 'Call ended';
    return (
      <div className="flex justify-center my-1">
        <span className="inline-flex items-center gap-1.5 rounded-full bg-[#1a1d21] px-3 py-1 text-xs text-[#71747a]">
          <span className="material-symbols-outlined text-[14px]">call</span>
          {label} · {formatTime(message.timestamp)}
        </span>
      </div>
    );
  }

  return (
    <div
      className={`flex gap-2.5 ${isOwn ? 'flex-row-reverse' : 'flex-row'} ${
        isPending ? 'opacity-60' : ''
      }`}
    >
      <div
        onClick={() => onAvatarClick?.(message.sender)}
        className="cursor-pointer hover:opacity-80 transition-opacity"
      >
        <Avatar
          src={profilePic}
          size="sm"
        />
      </div>

      <div className={`flex max-w-[70%] flex-col ${isOwn ? 'items-end' : 'items-start'}`}>
        <span className="mb-1 text-xs text-[#71747a]">{senderName}</span>

        <div className="group relative">
          <div
            className={`msg-content rounded-2xl px-4 py-2.5 text-sm leading-relaxed ${
              isOwn
                ? 'rounded-tr-sm bg-[var(--accent)] text-white'
                : 'rounded-tl-sm bg-[#1a1d21] text-[#e8eaed]'
            }`}
            dangerouslySetInnerHTML={{ __html: renderedHtml }}
          />
          {isOwn && !isPending && !isFailed && onDelete && (
            <button
              onClick={() => onDelete(message.id)}
              className="absolute -right-8 top-0.5 opacity-0 group-hover:opacity-100 transition-opacity p-1 hover:text-red-400 text-[#71747a]"
              title="Delete message"
            >
              <span className="material-symbols-outlined text-[18px]">delete</span>
            </button>
          )}
        </div>

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
