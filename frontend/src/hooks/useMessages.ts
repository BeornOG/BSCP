import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api, apiPost, apiUpload } from '../lib/api';
import type { Message } from '../types';

export function useMessages(chatId: string | null) {
  return useQuery<Message[]>({
    queryKey: ['messages', chatId],
    queryFn: () => api<Message[]>(`/api/messages/${encodeURIComponent(chatId!)}`),
    enabled: !!chatId,
    refetchInterval: 1000,
    select: (data) => [...data].sort((a, b) => a.timestamp - b.timestamp),
  });
}

interface SendMessageVars {
  receiver: string;
  text: string;
  chatId: string;
  currentUser: string;
}

export function useSendMessage() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (vars: SendMessageVars) =>
      apiPost('/api/messages/', { receiver: vars.receiver, text: vars.text }),
    onMutate: async (vars) => {
      await queryClient.cancelQueries({ queryKey: ['messages', vars.chatId] });
      const previous = queryClient.getQueryData<Message[]>(['messages', vars.chatId]);

      queryClient.setQueryData<Message[]>(['messages', vars.chatId], (old) => {
        if (!old) return old;
        const pending: Message = {
          id: `pending-${Date.now()}`,
          sender: vars.currentUser,
          receiver: vars.receiver,
          text: vars.text,
          timestamp: Date.now() / 1000,
          is_read: false,
        };
        return [...old, pending];
      });

      return { previous, chatId: vars.chatId };
    },
    onError: (_err, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData(['messages', context.chatId], context.previous);
      }
    },
    onSettled: (_data, _err, vars) => {
      queryClient.invalidateQueries({ queryKey: ['messages', vars.chatId] });
    },
  });
}

export function useUploadFile() {
  return useMutation({
    mutationFn: (file: File) => apiUpload<{ markdown: string; url: string }>('/api/upload/', file),
  });
}
