import { useEffect } from 'react';
import { api, apiPost } from '../lib/api';

function urlBase64ToUint8Array(base64String: string) {
  const padding = '='.repeat((4 - (base64String.length % 4)) % 4);
  const base64 = (base64String + padding).replace(/-/g, '+').replace(/_/g, '/');
  const rawData = window.atob(base64);
  const outputArray = new Uint8Array(rawData.length);
  for (let i = 0; i < rawData.length; ++i) {
    outputArray[i] = rawData.charCodeAt(i);
  }
  return outputArray;
}

export function usePushNotifications() {
  useEffect(() => {
    if (typeof window === 'undefined' || typeof Notification === 'undefined') return;
    if (!('serviceWorker' in navigator) || !('PushManager' in window)) return;

    const registerSubscription = async () => {
      if (Notification.permission === 'default') {
        const granted = await Notification.requestPermission();
        if (granted !== 'granted') return;
      }
      if (Notification.permission !== 'granted') return;

      const registration = await navigator.serviceWorker.register('/sw.js');
      const response = await api<{ publicKey: string }>('/api/users/push/vapid_public_key');
      if (!response.publicKey) return;

      const applicationServerKey = urlBase64ToUint8Array(response.publicKey);
      let subscription = await registration.pushManager.getSubscription();
      if (!subscription) {
        subscription = await registration.pushManager.subscribe({
          userVisibleOnly: true,
          applicationServerKey,
        });
      }

      const subscriptionJson = subscription.toJSON();
      await apiPost('/api/users/me/push/subscribe', {
        endpoint: subscriptionJson.endpoint,
        keys: subscriptionJson.keys,
      });
    };

    registerSubscription().catch((error) => {
      console.error('[PUSH] Registration failed', error);
    });
  }, []);
}

