import { useQuery, useMutation } from '@tanstack/react-query';
import { api } from '../lib/api';

function getCookie(name: string): string {
  const v = document.cookie.match('(^|;)\\s*' + name + '\\s*=\\s*([^;]+)');
  return v ? v.pop()! : '';
}

async function authFetch<T>(url: string, body: unknown): Promise<T> {
  const res = await fetch(url, {
    method: 'POST',
    credentials: 'include',
    headers: {
      'Content-Type': 'application/json',
      'X-CSRFToken': getCookie('csrftoken'),
    },
    body: JSON.stringify(body),
  });
  return res.json();
}

interface SetupStatus {
  needs_setup: boolean;
}

export function useAuthCheck(requireAuth = true) {
  return useQuery({
    queryKey: ['auth'],
    queryFn: async () => {
      const setupRes = await fetch('/api/auth/setup', { credentials: 'include' });
      const setupData: SetupStatus = await setupRes.json();

      if (setupData.needs_setup) {
        return { needsSetup: true, isAuthenticated: false };
      }

      if (!requireAuth) {
        return { needsSetup: false, isAuthenticated: false };
      }

      try {
        await api('/api/userprofile/');
        return { needsSetup: false, isAuthenticated: true };
      } catch {
        return { needsSetup: false, isAuthenticated: false };
      }
    },
    refetchOnWindowFocus: false,
  });
}

export function useLogin() {
  return useMutation({
    mutationFn: (data: { user: string; password: string }) =>
      authFetch<{ success: boolean; requires_2fa?: boolean; error?: string }>(
        '/api/auth/login',
        data
      ),
  });
}

export function useVerify2fa() {
  return useMutation({
    mutationFn: (data: { otp: string }) =>
      authFetch<{ success?: boolean; error?: string }>('/api/auth/2fa', data),
  });
}

export function useSetup() {
  return useMutation({
    mutationFn: (data: { username: string; email?: string; password: string; password_confirm: string }) =>
      authFetch<{ success?: boolean; errors?: string[] }>('/api/auth/setup', data),
  });
}

export function useRegister() {
  return useMutation({
    mutationFn: (data: { username: string; password: string; password_confirm: string; invite_code: string }) =>
      authFetch<{ success?: boolean; errors?: string[] }>('/api/auth/register', data),
  });
}

export function useLogout() {
  return useMutation({
    mutationFn: async () => {
      await fetch('/api/auth/logout', {
        method: 'POST',
        credentials: 'include',
        headers: { 'X-CSRFToken': getCookie('csrftoken') },
      });
    },
    onSuccess: () => {
      window.location.href = '/login';
    },
  });
}
