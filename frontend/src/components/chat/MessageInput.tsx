import { useState, useRef, useLayoutEffect, type FC, type KeyboardEvent, type ChangeEvent } from 'react';

interface MessageInputProps {
  onSend: (text: string) => void;
  onFileUpload: (file: File) => Promise<string | null>;
  disabled?: boolean;
  isWebhook?: boolean;
}

const MAX_HEIGHT = 24 * 10;

const MessageInput: FC<MessageInputProps> = ({ onSend, onFileUpload, disabled, isWebhook }) => {
  const [text, setText] = useState('');
  const [isUploading, setIsUploading] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const placeholder = isWebhook ? 'Cannot send messages to webhooks :)' : 'Type a message...';

  useLayoutEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = '0px';
    const scrollH = el.scrollHeight;
    el.style.height = `${Math.min(scrollH, MAX_HEIGHT)}px`;
    el.style.overflowY = scrollH > MAX_HEIGHT ? 'auto' : 'hidden';
  }, [text]);

  const handleSend = () => {
    const trimmed = text.trim();
    if (!trimmed) return;
    onSend(trimmed);
    setText('');
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleChange = (e: ChangeEvent<HTMLTextAreaElement>) => {
    setText(e.target.value);
  };

  const handleFileChange = async (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      setIsUploading(true);
      try {
        const markdown = await onFileUpload(file);
        if (markdown) {
          const newText = text ? `${text}\n\n${markdown}` : markdown;
          setText(newText);
        }
      } finally {
        setIsUploading(false);
      }
      e.target.value = '';
    }
  };

  return (
    <div className="sticky bottom-0 border-t border-[#232529] bg-[#141517] px-6 py-4">
      <div className="flex items-end gap-3">
        <button
          type="button"
          onClick={() => fileInputRef.current?.click()}
          disabled={disabled || isUploading}
          className="flex items-center justify-center text-[#71747a] transition-colors hover:text-[#e8eaed] disabled:opacity-50"
        >
          <span className="material-symbols-outlined text-[22px]">
            {isUploading ? 'hourglass_bottom' : 'attach_file'}
          </span>
        </button>
        <input
          ref={fileInputRef}
          type="file"
          className="hidden"
          onChange={handleFileChange}
        />

        <textarea
          ref={textareaRef}
          value={text}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          disabled={disabled || isUploading}
          placeholder={placeholder}
          style={{ minHeight: '24px', overflowY: 'hidden' }}
          className="flex-1 resize-none border-none bg-transparent text-sm leading-6 text-[#e8eaed] placeholder-[#71747a] outline-none disabled:opacity-50"
        />

        {text.trim() && (
          <button
            type="button"
            onClick={handleSend}
            disabled={disabled || isUploading}
            className="flex h-8 w-8 items-center justify-center rounded-full bg-[var(--accent)] text-white transition-opacity hover:opacity-90 disabled:opacity-50"
          >
            <span className="material-symbols-outlined text-[18px]">
              arrow_upward
            </span>
          </button>
        )}
      </div>
    </div>
  );
};

export default MessageInput;
