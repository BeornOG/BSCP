import { useCallback, useEffect, useRef } from 'react';
import { api } from '../lib/api';

const ACTIVITY_THROTTLE_MS = 15_000;
const ACTIVITY_EVENTS = [
  'mousemove',
  'mousedown',
  'keydown',
  'touchstart',
  'focus',
];

export function useUserActivityPing() {
  const lastPingRef = useRef(0);

  const sendActivityPing = useCallback(async () => {
    const now = Date.now();
    if (now - lastPingRef.current < ACTIVITY_THROTTLE_MS) {
      return;
    }
    lastPingRef.current = now;

    try {
      await api('/api/users/me/activity', {
        method: 'POST',
      });
    } catch {
      // ignore failures, status detection is best-effort
    }
  }, []);

  useEffect(() => {
    const handleActivity = () => {
      if (document.visibilityState !== 'visible') return;
      void sendActivityPing();
    };

    const handleVisibility = () => {
      if (document.visibilityState === 'visible') {
        void sendActivityPing();
      }
    };

    ACTIVITY_EVENTS.forEach((event) => window.addEventListener(event, handleActivity));
    document.addEventListener('visibilitychange', handleVisibility);

    // Send an initial ping on mount if user is already active.
    void sendActivityPing();

    return () => {
      ACTIVITY_EVENTS.forEach((event) => window.removeEventListener(event, handleActivity));
      document.removeEventListener('visibilitychange', handleVisibility);
    };
  }, [sendActivityPing]);

  return sendActivityPing;
}
