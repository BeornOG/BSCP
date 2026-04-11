import { useState, type FC, type KeyboardEvent } from 'react';
import { Button, Input, Avatar } from '../ui';
import type { Chat } from '../../types';

interface ChatSidebarProps {
  chats: Chat[];
  activeChatId: string | null;
  onSelectChat: (chat: Chat) => void;
  onNewChat: (receiver: string) => void;
  isLoading: boolean;
}

const ChatSidebar: FC<ChatSidebarProps> = ({
  chats,
  activeChatId,
  onSelectChat,
  onNewChat,
  isLoading,
}) => {
  const [showNewChat, setShowNewChat] = useState(false);
  const [newChatReceiver, setNewChatReceiver] = useState('');

  const handleNewChatSubmit = () => {
    const trimmed = newChatReceiver.trim();
    if (!trimmed) return;
    onNewChat(trimmed);
    setNewChatReceiver('');
    setShowNewChat(false);
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleNewChatSubmit();
    }
  };

  return (
    <div className="flex h-full w-72 flex-col border-r border-[#232529] bg-[#0a0a0b]">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4">
        <h2 className="text-lg font-semibold text-[#e8eaed]">Messages</h2>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setShowNewChat(!showNewChat)}
          icon={
            <span className="material-symbols-outlined text-[20px]">
              edit_square
            </span>
          }
        />
      </div>

      {/* New chat input row */}
      {showNewChat && (
        <div className="flex items-center gap-2 px-4 pb-3">
          <Input
            placeholder="Username..."
            value={newChatReceiver}
            onChange={(e) => setNewChatReceiver(e.target.value)}
            onKeyDown={handleKeyDown}
            className="!py-2"
          />
          <Button
            variant="primary"
            size="sm"
            onClick={handleNewChatSubmit}
            disabled={!newChatReceiver.trim()}
            icon={
              <span className="material-symbols-outlined text-[18px]">
                send
              </span>
            }
          />
        </div>
      )}

      {/* Chat list */}
      <div className="flex-1 overflow-y-auto">
        {isLoading && chats.length === 0 && (
          <p className="px-5 py-4 text-sm text-[#71747a]">Loading...</p>
        )}
        {chats.map((chat) => (
          <button
            key={chat.id}
            onClick={() => onSelectChat(chat)}
            className={`flex w-full items-center gap-3 px-5 py-3 text-left transition-colors duration-150 hover:bg-[#141517] ${
              activeChatId === chat.id ? 'bg-[#1a1d21]' : ''
            }`}
          >
            <Avatar size="sm" />
            <span className="truncate text-sm font-medium text-[#e8eaed]">
              {chat.display_name}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
};

export default ChatSidebar;
