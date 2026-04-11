import { useState, useMemo } from 'react';
import { useChats } from '../hooks/useChats';
import { useMessages, useSendMessage, useUploadFile } from '../hooks/useMessages';
import { useProfile } from '../hooks/useProfile';
import ChatSidebar from '../components/chat/ChatSidebar';
import MessageList from '../components/chat/MessageList';
import MessageInput from '../components/chat/MessageInput';
import { Avatar, Modal } from '../components/ui';
import type { UserStatus } from '../components/ui/Avatar';

export default function ChatPage() {
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [activeChatName, setActiveChatName] = useState('');
  const [activeChatPic, setActiveChatPic] = useState<string | null>(null);
  const [activeChatStatus, setActiveChatStatus] = useState<UserStatus>('offline');
  const [modalImage, setModalImage] = useState<string | null>(null);

  const { data: chats, isLoading: chatsLoading } = useChats();
  const { data: serverMessages, isLoading: messagesLoading } = useMessages(activeChatId);
  const sendMessage = useSendMessage();

  const messages = useMemo(() => {
    const server = serverMessages || [];
    // Filter out pending messages that already appear in server data
    const local = sendMessage.localMessages.filter((lm) =>
      lm.id.startsWith('failed-') ||
      !server.some((sm) => sm.sender === lm.sender && sm.text === lm.text)
    );
    return [...server, ...local].sort((a, b) => a.timestamp - b.timestamp);
  }, [serverMessages, sendMessage.localMessages]);
  const uploadFile = useUploadFile();
  const { data: profile } = useProfile();

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

  const handleFileUpload = (file: File) => {
    uploadFile.mutate(file, {
      onSuccess: (data) => {
        if (data.markdown && activeChatId && profile) {
          sendMessage.mutate({
            text: data.markdown,
            chatId: activeChatId,
            currentUser: profile.username,
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
              <Avatar src={activeChatPic} name={activeChatName} size="sm" status={activeChatStatus} />
              <div className="min-w-0">
                <h2 className="text-[#e8eaed] font-medium truncate">{activeChatName}</h2>
                {activeChatId !== activeChatName && (
                  <p className="text-[#71747a] text-xs truncate">{activeChatId}</p>
                )}
              </div>
            </div>

            <MessageList
              messages={messages}
              currentUser={profile?.username || ''}
              isLoading={messagesLoading}
              onImageClick={(src) => setModalImage(src)}
            />

            <MessageInput
              onSend={handleSend}
              onFileUpload={handleFileUpload}
              disabled={uploadFile.isPending}
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
