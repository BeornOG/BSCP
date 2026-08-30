import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api';

export interface OAuthClient {
  client_id: string;
  name: string;
  redirect_uris: string[];
  token_endpoint_auth_method: string;
  created_at: number;
  disabled: boolean;
}

export function useOAuthConfig() {
  return useQuery<{ oidc_enabled: boolean }>({
    queryKey: ['oauth', 'config'],
    queryFn: () => api('/api/admin/oauth/config'),
    refetchOnWindowFocus: false,
  });
}

export function useSetOAuthEnabled() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (oidc_enabled: boolean) =>
      api('/api/admin/oauth/config', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ oidc_enabled }),
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['oauth'] }),
  });
}

export function useOAuthClients() {
  return useQuery<OAuthClient[]>({
    queryKey: ['oauth', 'clients'],
    queryFn: () => api('/api/admin/oauth/clients'),
    refetchOnWindowFocus: false,
  });
}

export function useRevokeOAuthClient() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (clientId: string) =>
      api(`/api/admin/oauth/clients/${encodeURIComponent(clientId)}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['oauth', 'clients'] }),
  });
}
