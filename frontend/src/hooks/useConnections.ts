import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api';

export interface Provider {
  module: string;
  id: string;
  name: string;
  icon_url?: string | null;
  linked: boolean;
  link?: {
    display_name?: string | null;
    profile_url?: string | null;
    avatar_url?: string | null;
  } | null;
}

export function useConnections() {
  return useQuery<Provider[]>({
    queryKey: ['connections'],
    queryFn: () => api('/api/modules/providers'),
    refetchOnWindowFocus: false,
  });
}

/** Full-page redirect into the module's OAuth flow. */
export function startLink(module: string, provider: string) {
  window.location.assign(
    `/api/modules/${encodeURIComponent(module)}/link/${encodeURIComponent(provider)}/start`,
  );
}

export function useUnlink() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ module, provider }: { module: string; provider: string }) =>
      api(`/api/modules/${encodeURIComponent(module)}/links/${encodeURIComponent(provider)}`, {
        method: 'DELETE',
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['connections'] }),
  });
}
