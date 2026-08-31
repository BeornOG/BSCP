import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api';

export interface BlockedDomain {
  domain: string;
  reason: string | null;
  blocked_by: string | null;
  created_at: number;
}

export function useBlockedDomains() {
  return useQuery<BlockedDomain[]>({
    queryKey: ['admin', 'blocked-domains'],
    queryFn: () => api('/api/admin/blocked-domains'),
    refetchOnWindowFocus: false,
  });
}

export function useBlockDomain() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { domain: string; reason?: string }) =>
      api('/api/admin/blocked-domains', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin', 'blocked-domains'] }),
  });
}

export function useUnblockDomain() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (domain: string) =>
      api(`/api/admin/blocked-domains/${encodeURIComponent(domain)}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin', 'blocked-domains'] }),
  });
}
