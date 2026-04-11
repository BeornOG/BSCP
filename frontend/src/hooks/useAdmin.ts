import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api';
import { useProfile } from './useProfile';
import type { UserProfile, Invite } from '../types';

export function useUsers() {
  return useQuery<UserProfile[]>({
    queryKey: ['admin', 'users'],
    queryFn: () => api<UserProfile[]>('/api/users/'),
  });
}

export function useIsAdmin() {
  const { data: profile } = useProfile();
  return { data: profile?.is_admin ?? false };
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
    mutationFn: (fullId: string) => api(`/api/users/${encodeURIComponent(fullId)}`, { method: 'DELETE' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin', 'users'] });
    },
  });
}
