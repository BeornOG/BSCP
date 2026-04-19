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

let audioContext: AudioContext | null = null;
let activeChatId: string | null = null;

export function setActiveChatId(chatId: string | null) {
  activeChatId = chatId;
}

export function initAudioContext() {
  if (audioContext) return;
  try {
    audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();

    if (audioContext.state === 'suspended') {
      const resume = () => {
        audioContext?.resume().catch(() => {});
        document.removeEventListener('click', resume);
        document.removeEventListener('keypress', resume);
      };
      document.addEventListener('click', resume);
      document.addEventListener('keypress', resume);
    }
  } catch (err) {
    // Silent fail - audio context might not be available
  }
}

function playChime() {
  try {
    if (!audioContext) return;
    if (audioContext.state !== 'running') return;

    const enableChime = localStorage.getItem('notif_chime') !== 'false';
    if (!enableChime) return;

    const now = audioContext.currentTime;
    const oscillator = audioContext.createOscillator();
    const gainNode = audioContext.createGain();

    oscillator.connect(gainNode);
    gainNode.connect(audioContext.destination);

    oscillator.frequency.value = 800;
    oscillator.type = 'sine';

    gainNode.gain.setValueAtTime(0.3, now);
    gainNode.gain.exponentialRampToValueAtTime(0.01, now + 0.3);

    oscillator.start(now);
    oscillator.stop(now + 0.3);
  } catch (err) {
    // Silent fail
  }
}

export function useMessageNotifications() {
  const { data: chats } = useChats();
  const prevUnread = useRef<Record<string, number> | null>(null);
  const totalUnread = useRef(0);

  useEffect(() => {
    if (typeof Notification !== 'undefined' && Notification.permission === 'default') {
      Notification.requestPermission().catch(() => {});
    }
  }, []);

  useEffect(() => {
    if (!chats) return;

    const counts = Object.fromEntries(chats.map((c) => [c.id, c.unread_count]));
    const newTotal = Object.values(counts).reduce((sum, count) => sum + count, 0);

    if (prevUnread.current === null) {
      prevUnread.current = counts;
      totalUnread.current = newTotal;
      updateTabBadge(newTotal);
      return;
    }

    const prev = prevUnread.current;

    for (const chat of chats) {
      const prevCount = prev[chat.id] ?? 0;
      if (chat.unread_count > prevCount) {
        playChime();

        const isActiveChat = chat.id === activeChatId;
        const shouldNotify = document.hidden || !isActiveChat;
        const enableDesktopNotif = localStorage.getItem('notif_desktop') !== 'false';

        if (shouldNotify && enableDesktopNotif && Notification.permission === 'granted') {
          const title = chat.display_name;
          const body = 'New message';
          const options: NotificationOptions = {
            body,
            tag: `chat-${chat.id}`,
            renotify: true,
          } as NotificationOptions;

          try {
            const notif = new Notification(title, options);
            notif.onclick = () => {
              window.focus();
              notif.close();
            };
          } catch (err) {
            console.error('[NOTIF] Failed to show notification:', err);
          }
        }
      }
    }

    prevUnread.current = counts;
    totalUnread.current = newTotal;
    updateTabBadge(newTotal);
  }, [chats]);

  useEffect(() => {
    const handleVisibilityChange = () => {
      if (!document.hidden && totalUnread.current === 0) {
        document.title = 'Chat';
      }
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => document.removeEventListener('visibilitychange', handleVisibilityChange);
  }, []);
}

function updateTabBadge(count: number) {
  if (typeof document === 'undefined') return;
  const baseTitle = 'Chat';
  document.title = count > 0 ? `(${count}) ${baseTitle}` : baseTitle;
}
