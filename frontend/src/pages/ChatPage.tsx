import { useState, useMemo, useEffect } from 'react';
import { useChats } from '../hooks/useChats';
import { useMessages, useSendMessage, useUploadFile, useDeleteMessage } from '../hooks/useMessages';
import { useProfile } from '../hooks/useProfile';
import { setActiveChatId as notifyActiveChatId } from '../hooks/useNotifications';
import ChatSidebar from '../components/chat/ChatSidebar';
import MessageList from '../components/chat/MessageList';
import MessageInput from '../components/chat/MessageInput';
import { Avatar, Modal, ProfileModal } from '../components/ui';
import type { UserStatus } from '../components/ui/Avatar';
import { api } from '../lib/api';
import type { UserProfile } from '../types';

export default function ChatPage() {
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [activeChatName, setActiveChatName] = useState('');
  const [activeChatPic, setActiveChatPic] = useState<string | null>(null);
  const [activeChatStatus, setActiveChatStatus] = useState<UserStatus>('offline');
  const [modalImage, setModalImage] = useState<string | null>(null);
  const [profileModal, setProfileModal] = useState<{ open: boolean; userId: string | null }>({ open: false, userId: null });
  const [profileData, setProfileData] = useState<UserProfile | null>(null);
  const [profileLoading, setProfileLoading] = useState(false);

  const { data: chats, isLoading: chatsLoading } = useChats();
  const { data: serverMessages, isLoading: messagesLoading } = useMessages(activeChatId);
  const sendMessage = useSendMessage();
  const deleteMessage = useDeleteMessage(activeChatId);
  const { data: profile } = useProfile();

  const messages = useMemo(() => {
    const server = serverMessages || [];
    const local = sendMessage.localMessages.filter((lm) =>
      lm.id.startsWith('failed-') ||
      !server.some((sm) => sm.sender === lm.sender && sm.text === lm.text)
    );
    const allMessages = [...server, ...local].sort((a, b) => a.timestamp - b.timestamp);

    const currentChat = chats?.find((c) => c.id === activeChatId);
    if (currentChat && currentChat.unread_count > 0) {
      const unreadCount = currentChat.unread_count;
      const startIndex = Math.max(0, allMessages.length - unreadCount);
      return allMessages.map((msg, idx) => ({
        ...msg,
        is_read: idx < startIndex,
      }));
    }

    return allMessages;
  }, [serverMessages, sendMessage.localMessages, chats, activeChatId]);
  const uploadFile = useUploadFile();

  useEffect(() => {
    notifyActiveChatId(activeChatId);
  }, [activeChatId]);

  const handleSelectChat = (chat: { id: string; display_name: string; profile_pic: string | null; status: UserStatus }) => {
    sendMessage.clearFailed();
    setActiveChatId(chat.id);
    setActiveChatName(chat.display_name);
    setActiveChatPic(chat.profile_pic);
    setActiveChatStatus(chat.status);
  };

  const handleNewChat = (receiver: string) => {
    sendMessage.clearFailed();
    setActiveChatId(receiver);
    setActiveChatName(receiver);
    setActiveChatPic(null);
    setActiveChatStatus('offline');
  };

  const handleSend = (text: string) => {
    if (!activeChatId || !profile) return;
    sendMessage.mutate({
      text,
      chatId: activeChatId,
      currentUser: profile.username,
    });
  };

  const handleFileUpload = async (file: File): Promise<string | null> => {
    try {
      const data = await uploadFile.mutateAsync(file);
      return data.markdown || null;
    } catch {
      return null;
    }
  };

  const handleDeleteMessage = (messageId: string) => {
    deleteMessage.mutate(messageId);
  };

  const handleOpenProfile = async (userId: string) => {
    setProfileModal({ open: true, userId });
    setProfileLoading(true);
    try {
      const data = await api<UserProfile>(`/api/users/${userId}`);
      setProfileData(data);
    } catch (error) {
      console.error('Failed to fetch profile:', error);
    } finally {
      setProfileLoading(false);
    }
  };

  return (
    <div className="flex h-full">
      <ChatSidebar
        chats={chats || []}
        activeChatId={activeChatId}
        onSelectChat={handleSelectChat}
        onNewChat={handleNewChat}
        isLoading={chatsLoading}
      />

      <div className="flex-1 flex flex-col min-w-0">
        {!activeChatId ? (
          <div className="flex-1 flex items-center justify-center">
            <p className="text-[#71747a] text-lg">Select a conversation</p>
          </div>
        ) : (
          <>
            <div className="flex items-center gap-3 px-6 py-4 border-b border-[#232529]">
              <div
                onClick={() => activeChatId && handleOpenProfile(activeChatId)}
                className="cursor-pointer hover:opacity-80 transition-opacity"
              >
                <Avatar src={activeChatPic} name={activeChatName} size="sm" status={activeChatStatus} />
              </div>
              <div className="min-w-0">
                <h2 className="text-[#e8eaed] font-medium truncate">{activeChatName}</h2>
                {activeChatId !== activeChatName && (
                  <p className="text-[#71747a] text-xs truncate">{activeChatId}</p>
                )}
              </div>
            </div>

            <MessageList
              chatId={activeChatId}
              messages={messages}
              currentUser={profile?.username || ''}
              isLoading={messagesLoading}
              onImageClick={(src) => setModalImage(src)}
              onAvatarClick={handleOpenProfile}
              onDeleteMessage={handleDeleteMessage}
            />

            <MessageInput
              onSend={handleSend}
              onFileUpload={handleFileUpload}
              disabled={activeChatId?.startsWith('webhook-')}
              isWebhook={activeChatId?.startsWith('webhook-')}
            />
          </>
        )}
      </div>

      {modalImage && (
        <Modal onClose={() => setModalImage(null)}>
          <img src={modalImage} alt="Preview" className="max-w-full max-h-[80vh] rounded-lg" />
        </Modal>
      )}

      <ProfileModal
        isOpen={profileModal.open}
        onClose={() => setProfileModal({ open: false, userId: null })}
        profile={profileData || undefined}
        isLoading={profileLoading}
      />
    </div>
  );
}
