import { useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api';

export interface TwoFactorSetupResponse {
  secret: string;
  qr_code: string;
  provisioning_uri: string;
}

export function useTwoFactorSetup() {
  return useMutation({
    mutationFn: () =>
      api<TwoFactorSetupResponse>('/api/users/me/2fa/setup', {
        method: 'POST',
      }),
  });
}

export function useTwoFactorEnable() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (otp: string) =>
      api('/api/users/me/2fa/enable', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ otp }),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['profile'], refetchType: 'all' });
    },
  });
}

export function useTwoFactorDisable() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (password: string) =>
      api('/api/users/me/2fa/disable', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ password }),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['profile'], refetchType: 'all' });
    },
  });
}
