import { useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api';

export interface UserStorageConfig {
  user_id: string;
  username: string;
  storage_limit_mb: number;
}

export function useUpdateUserStorageLimit() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ username, limit_mb }: { username: string; limit_mb: number }) =>
      api<UserStorageConfig>(`/api/admin/users/${encodeURIComponent(username)}/storage`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ storage_limit_mb: limit_mb }),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin', 'users'] });
      queryClient.invalidateQueries({ queryKey: ['users'] });
    },
  });
}
