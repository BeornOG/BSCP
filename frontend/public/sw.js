self.addEventListener('push', (event) => {
  let payload = {};
  try {
    payload = event.data ? event.data.json() : {};
  } catch (err) {
    payload = { body: event.data?.text() || '' };
  }

  const title = payload.title || 'New notification';
  const body = payload.body || '';
  const url = payload.url || '/';
  const tag = payload.tag || undefined;

  const show = async () => {
    await self.registration.showNotification(title, {
      body,
      tag,
      data: { url },
      renotify: true,
    });
  };

  event.waitUntil(show());
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  const url = event.notification.data?.url || '/';

  event.waitUntil(
    self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then((clientList) => {
      for (const client of clientList) {
        if ('focus' in client) {
          client.focus();
          return client.navigate(url);
        }
      }
      if (self.clients.openWindow) {
        return self.clients.openWindow(url);
      }
      return null;
    }),
  );
});
