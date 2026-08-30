import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api, apiPost } from '../lib/api';

export interface ModuleInfo {
  name: string;
  base_url: string;
  enabled: boolean;
  manifest: {
    description?: string;
    version?: string;
    events: string[];
    link_providers: { id: string; name: string; icon_url?: string }[];
    admin_url?: string | null;
  };
}

export interface AddModuleResult {
  name: string;
  base_url: string;
  secret: string;
  events: string[];
  link_providers: { id: string; name: string }[];
}

export function useModules() {
  return useQuery<ModuleInfo[]>({
    queryKey: ['modules'],
    queryFn: () => api('/api/admin/modules'),
    refetchOnWindowFocus: false,
  });
}

export function useAddModule() {
  const qc = useQueryClient();
  return useMutation<AddModuleResult, Error, string>({
    mutationFn: (base_url: string) => apiPost('/api/admin/modules', { base_url }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['modules'] }),
  });
}

export function useSetModuleEnabled() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ name, enabled }: { name: string; enabled: boolean }) =>
      api(`/api/admin/modules/${encodeURIComponent(name)}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled }),
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['modules'] }),
  });
}

export function useRemoveModule() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (name: string) =>
      api(`/api/admin/modules/${encodeURIComponent(name)}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['modules'] }),
  });
}
