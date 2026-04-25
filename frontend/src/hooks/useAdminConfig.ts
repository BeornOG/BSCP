import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api';

export interface ServerConfig {
  storage_limit_mb: number;
}

export function useAdminConfig() {
  return useQuery<ServerConfig>({
    queryKey: ['admin:config'],
    queryFn: () => api<ServerConfig>('/api/admin/config'),
    refetchOnWindowFocus: false,
  });
}

export function useUpdateAdminConfig() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: { storage_limit_mb: number }) =>
      api<ServerConfig>('/api/admin/config', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin:config'] });
      queryClient.invalidateQueries({ queryKey: ['uploads'] });
    },
  });
}
