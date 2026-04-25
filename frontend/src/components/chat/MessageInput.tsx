import { useState, useRef, type FC, type KeyboardEvent, type ChangeEvent } from 'react';

interface MessageInputProps {
  onSend: (text: string) => void;
  onFileUpload: (file: File) => void;
  disabled?: boolean;
  isWebhook?: boolean;
}

const MessageInput: FC<MessageInputProps> = ({ onSend, onFileUpload, disabled, isWebhook }) => {
  const [text, setText] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  const placeholder = isWebhook ? 'Cannot send messages to webhooks :)' : 'Type a message...';

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

  const handleFileChange = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      onFileUpload(file);
      e.target.value = '';
    }
  };

  return (
    <div className="sticky bottom-0 border-t border-[#232529] bg-[#141517] px-6 py-4">
      <div className="flex items-center gap-3">
        {/* File upload button */}
        <button
          type="button"
          onClick={() => fileInputRef.current?.click()}
          disabled={disabled}
          className="flex items-center justify-center text-[#71747a] transition-colors hover:text-[#e8eaed] disabled:opacity-50"
        >
          <span className="material-symbols-outlined text-[22px]">
            attach_file
          </span>
        </button>
        <input
          ref={fileInputRef}
          type="file"
          className="hidden"
          onChange={handleFileChange}
        />

        {/* Text input */}
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={disabled}
          placeholder={placeholder}
          rows={1}
          className="flex-1 resize-none border-none bg-transparent text-sm text-[#e8eaed] placeholder-[#71747a] outline-none disabled:opacity-50"
        />

        {/* Send button */}
        {text.trim() && (
          <button
            type="button"
            onClick={handleSend}
            disabled={disabled}
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
