/// <reference no-default-lib="true" />
/// <reference lib="esnext" />
/// <reference lib="webworker" />
/// <reference types="@sveltejs/kit" />

import { base, build, files, version } from "$service-worker";

const worker = globalThis.self as unknown as ServiceWorkerGlobalScope;
const CACHE_PREFIX = "pandan-shell-";
const CACHE = `${CACHE_PREFIX}${version}`;
const OFFLINE_PAGE = `${base}/offline.html`;
const API_PREFIX = `${base}/api/`;
const STATIC_ASSETS = files.filter((path) => !path.endsWith("/og-card.png"));
const PRECACHE = [...new Set([...build, ...STATIC_ASSETS, OFFLINE_PAGE])];
const PRECACHE_PATHS = new Set(
  PRECACHE.map((path) => new URL(path, worker.location.origin).pathname),
);

worker.addEventListener("install", (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(PRECACHE)));
});

worker.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys.map((key) =>
            key.startsWith(CACHE_PREFIX) && key !== CACHE
              ? caches.delete(key)
              : Promise.resolve(false),
          ),
        ),
      )
      .then(() => worker.clients.claim()),
  );
});

worker.addEventListener("message", (event) => {
  if (event.data?.type === "SKIP_WAITING") void worker.skipWaiting();
});

worker.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const data = event.notification.data;
  if (
    data?.type !== "PANDAN_OPEN_NOTIFICATION" ||
    typeof data.userId !== "string" ||
    typeof data.notificationId !== "string"
  )
    return;

  event.waitUntil(
    (async () => {
      const clients = await worker.clients.matchAll({
        type: "window",
        includeUncontrolled: true,
      });
      const appUrl = new URL(`${base}/`, worker.location.origin);
      const candidates = clients.filter((client) => {
        const url = new URL(client.url);
        return url.origin === appUrl.origin && url.pathname === appUrl.pathname;
      });
      const client =
        candidates.find((candidate) => candidate.focused) ?? candidates[0];
      if (client) {
        // The authenticated shell checks the account before opening the record.
        client.postMessage(data);
        await client.focus();
        return;
      }
      appUrl.hash = new URLSearchParams({
        notification: data.notificationId,
        "notification-user": data.userId,
      }).toString();
      await worker.clients.openWindow(appUrl.href);
    })(),
  );
});

worker.addEventListener("fetch", (event) => {
  const { request } = event;
  if (request.method !== "GET") return;

  const url = new URL(request.url);
  if (
    url.origin !== worker.location.origin ||
    url.pathname.startsWith(API_PREFIX)
  ) {
    return;
  }

  if (request.mode === "navigate") {
    event.respondWith(
      fetch(request).catch(async () => {
        const fallback = await caches.match(OFFLINE_PAGE);
        return fallback ?? Response.error();
      }),
    );
    return;
  }

  if (!PRECACHE_PATHS.has(url.pathname)) return;

  event.respondWith(
    caches.match(url.pathname).then((cached) => cached ?? fetch(request)),
  );
});
