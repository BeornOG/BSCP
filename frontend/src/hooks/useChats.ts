import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api, apiPost } from '../lib/api';
import type { Chat } from '../types';

export function useChats() {
  return useQuery<Chat[]>({
    queryKey: ['chats'],
    queryFn: () => api<Chat[]>('/api/chats/'),
    refetchInterval: 2000,
  });
}

export function useStartChat() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (receiver: string) =>
      apiPost('/api/messages/', { receiver, text: 'Hello!' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['chats'] });
    },
  });
}
