import { useState } from 'react';
import { useChats } from '../hooks/useChats';
import { useMessages, useSendMessage, useUploadFile } from '../hooks/useMessages';
import { useProfile } from '../hooks/useProfile';
import ChatSidebar from '../components/chat/ChatSidebar';
import MessageList from '../components/chat/MessageList';
import MessageInput from '../components/chat/MessageInput';
import { Avatar, Modal } from '../components/ui';

export default function ChatPage() {
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [activeChatName, setActiveChatName] = useState('');
  const [modalImage, setModalImage] = useState<string | null>(null);

  const { data: chats, isLoading: chatsLoading } = useChats();
  const { data: messages, isLoading: messagesLoading } = useMessages(activeChatId);
  const sendMessage = useSendMessage();
  const uploadFile = useUploadFile();
  const { data: profile } = useProfile();

  const handleSelectChat = (chatId: string) => {
    setActiveChatId(chatId);
    const chat = chats?.find((c) => c.id === chatId);
    if (chat) setActiveChatName(chat.display_name);
  };

  const handleNewChat = (receiver: string) => {
    setActiveChatName(receiver);
    setActiveChatId(null);
  };

  const handleSend = (text: string) => {
    if (!activeChatId || !profile) return;
    sendMessage.mutate({
      receiver: activeChatName,
      text,
      chatId: activeChatId,
      currentUser: profile.full_id,
    });
  };

  const handleFileUpload = (file: File) => {
    uploadFile.mutate(file, {
      onSuccess: (data) => {
        if (data.markdown && activeChatId && profile) {
          sendMessage.mutate({
            receiver: activeChatName,
            text: data.markdown,
            chatId: activeChatId,
            currentUser: profile.full_id,
          });
        }
      },
    });
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
              <Avatar name={activeChatName} size="sm" />
              <h2 className="text-[#e8eaed] font-medium truncate">{activeChatName}</h2>
            </div>

            <MessageList
              messages={messages || []}
              currentUser={profile?.full_id || ''}
              isLoading={messagesLoading}
              onImageClick={(src) => setModalImage(src)}
            />

            <MessageInput
              onSend={handleSend}
              onFileUpload={handleFileUpload}
              disabled={sendMessage.isPending || uploadFile.isPending}
            />
          </>
        )}
      </div>

      {modalImage && (
        <Modal onClose={() => setModalImage(null)}>
          <img src={modalImage} alt="Preview" className="max-w-full max-h-[80vh] rounded-lg" />
        </Modal>
      )}
    </div>
  );
}
