import { useCallback, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api, apiPost, apiUpload } from '../lib/api';
import type { Message } from '../types';

export function useMessages(chatId: string | null) {
  return useQuery<Message[]>({
    queryKey: ['messages', chatId],
    queryFn: () => api<Message[]>(`/api/chats/${encodeURIComponent(chatId!)}/messages`),
    enabled: !!chatId,
    refetchInterval: 1000,
    select: (data) => [...data].sort((a, b) => a.timestamp - b.timestamp),
  });
}

interface SendMessageVars {
  text: string;
  chatId: string;
  currentUser: string;
}

export function useSendMessage() {
  const queryClient = useQueryClient();
  const [localMessages, setLocalMessages] = useState<Message[]>([]);

  const mutation = useMutation({
    mutationFn: (vars: SendMessageVars) =>
      apiPost(`/api/chats/${encodeURIComponent(vars.chatId)}/messages`, { text: vars.text }),
    onMutate: async (vars) => {
      const id = `pending-${Date.now()}`;
      const pending: Message = {
        id,
        sender: vars.currentUser,
        receiver: vars.chatId,
        text: vars.text,
        timestamp: Date.now() / 1000,
        is_read: false,
      };
      setLocalMessages((prev) => [...prev, pending]);
      return { pendingId: id };
    },
    onSuccess: (_data, vars, context) => {
      if (context?.pendingId) {
        setLocalMessages((prev) => prev.filter((m) => m.id !== context.pendingId));
      }
      queryClient.invalidateQueries({ queryKey: ['messages', vars.chatId] });
    },
    onError: (err, _vars, context) => {
      if (context?.pendingId) {
        const errorMsg = err instanceof Error ? err.message : 'Failed to send';
        setLocalMessages((prev) =>
          prev.map((m) =>
            m.id === context.pendingId
              ? { ...m, id: `failed-${Date.now()}`, text: `${m.text}\n\n_${errorMsg}_` }
              : m
          )
        );
      }
    },
  });

  const clearFailed = useCallback(() => {
    setLocalMessages((prev) => prev.filter((m) => !m.id.startsWith('failed-')));
  }, []);

  return { ...mutation, localMessages, clearFailed };
}

export function useUploadFile() {
  return useMutation({
    mutationFn: (file: File) => apiUpload<{ markdown: string; url: string }>('/api/upload/', file),
  });
}
