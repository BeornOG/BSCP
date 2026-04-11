import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api';
import type { AdminUser, Invite } from '../types';

export function useUsers() {
  return useQuery<AdminUser[]>({
    queryKey: ['admin', 'users'],
    queryFn: () => api<AdminUser[]>('/api/users/'),
  });
}

export function useInvites() {
  return useQuery<Invite[]>({
    queryKey: ['admin', 'invites'],
    queryFn: () => api<Invite[]>('/api/invites/'),
  });
}

export function useGenerateInvite() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => api('/api/invites/generate', { method: 'POST' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin', 'invites'] });
    },
  });
}

export function useDeleteUser() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (userId: string) => api(`/api/users/${userId}`, { method: 'DELETE' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin', 'users'] });
    },
  });
}
