import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api';

export interface Upload {
  id: string;
  filename: string;
  mimetype: string;
  size_bytes: number;
  created_at: number;
}

export interface UserUploads {
  uploads: Upload[];
  total_size_bytes: number;
  limit_bytes: number;
}

export function useUploads() {
  return useQuery<UserUploads>({
    queryKey: ['uploads'],
    queryFn: () => api<UserUploads>('/api/upload/user/list'),
    refetchOnWindowFocus: false,
  });
}

export function useDeleteUpload() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (uploadId: string) =>
      api(`/api/upload/${uploadId}`, { method: 'DELETE' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['uploads'] });
    },
  });
}
