import { useEffect, useRef } from 'react';
import { api, apiPost } from '../lib/api';
import { useChats } from './useChats';

const VAPID_KEY_STORAGE = 'vapid_public_key';

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

      const storedKey = localStorage.getItem(VAPID_KEY_STORAGE);
      let subscription = await registration.pushManager.getSubscription();

      // VAPID key changed — existing subscription is stale, drop it
      if (subscription && storedKey !== response.publicKey) {
        await subscription.unsubscribe();
        subscription = null;
      }

      if (!subscription) {
        const applicationServerKey = urlBase64ToUint8Array(response.publicKey);
        subscription = await registration.pushManager.subscribe({
          userVisibleOnly: true,
          applicationServerKey,
        });
      }

      localStorage.setItem(VAPID_KEY_STORAGE, response.publicKey);

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

export function useMessageNotifications() {
  const { data: chats } = useChats();
  const prevUnread = useRef<Record<string, number> | null>(null);

  useEffect(() => {
    if (typeof Notification !== 'undefined' && Notification.permission === 'default') {
      Notification.requestPermission().catch(() => {});
    }
  }, []);

  useEffect(() => {
    if (!chats) return;

    const counts = Object.fromEntries(chats.map((c) => [c.id, c.unread_count]));

    if (prevUnread.current === null) {
      prevUnread.current = counts;
      return;
    }

    if (Notification.permission !== 'granted') {
      prevUnread.current = counts;
      return;
    }

    const prev = prevUnread.current;
    for (const chat of chats) {
      const prevCount = prev[chat.id] ?? 0;
      if (chat.unread_count > prevCount) {
        const title = chat.display_name;
        const options: NotificationOptions = { body: 'New message', tag: `chat-${chat.id}` };
        navigator.serviceWorker.getRegistration()
          .then((reg) => {
            if (reg) return reg.showNotification(title, options);
            new Notification(title, options);
          })
          .catch(() => { new Notification(title, options); });
      }
    }

    prevUnread.current = counts;
  }, [chats]);
}
