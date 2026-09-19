import type { NtfyNotification } from "./api";

const MESSAGE_TYPE = "PANDAN_OPEN_NOTIFICATION";
const HISTORY_LIMIT = 200;

export function browserNotificationPermission():
  NotificationPermission | "unsupported" {
  if (
    typeof window === "undefined" ||
    !window.isSecureContext ||
    !("Notification" in window)
  ) {
    return "unsupported";
  }
  return Notification.permission;
}

// One instance belongs to the authenticated shell, never to a topic page.
export class BrowserNotifications {
  #disposed = false;
  #enabled = false;
  #shown = new Set<string>();
  #native = new Set<Notification>();
  #preferenceKey: string;
  #historyKey: string;

  constructor(
    private userId: string,
    private onOpen: (id: string) => void,
  ) {
    this.#preferenceKey = `pandan-browser-notifications:${userId}`;
    this.#historyKey = `${this.#preferenceKey}:shown`;
    this.enabled();
    navigator.serviceWorker?.addEventListener("message", this.#handleMessage);
    window.addEventListener("hashchange", this.#handleHash);
    this.#handleHash();
  }

  enabled() {
    try {
      this.#enabled = localStorage.getItem(this.#preferenceKey) === "true";
    } catch {
      // Keep the current session usable when browser storage is unavailable.
    }
    return this.#enabled && browserNotificationPermission() === "granted";
  }

  async enable() {
    const permission = browserNotificationPermission();
    if (permission === "unsupported" || permission === "denied") return false;
    // This must run directly from a click, before any other asynchronous work.
    const result =
      permission === "granted"
        ? permission
        : await Notification.requestPermission();
    if (this.#disposed || result !== "granted") return false;
    this.#setEnabled(true);
    return true;
  }

  disable() {
    this.#setEnabled(false);
  }

  #setEnabled(enabled: boolean) {
    this.#enabled = enabled;
    try {
      localStorage.setItem(this.#preferenceKey, String(enabled));
    } catch {
      // A preference can still apply for this session without local storage.
    }
  }

  async show(notification: NtfyNotification, count = 1) {
    if (this.#disposed || !this.enabled()) return;
    try {
      // Serialise the claim across tabs so one delivery produces one OS alert.
      if (navigator.locks) {
        await navigator.locks.request(this.#historyKey, () =>
          this.#show(notification, count),
        );
      } else {
        await this.#show(notification, count);
      }
    } catch {
      // OS notification failures must never interrupt the inbox or its toast.
    }
  }

  async #show(notification: NtfyNotification, count: number) {
    if (this.#disposed || !this.enabled()) return;
    try {
      const stored: unknown = JSON.parse(
        localStorage.getItem(this.#historyKey) ?? "[]",
      );
      if (Array.isArray(stored)) {
        for (const id of stored.slice(-HISTORY_LIMIT)) {
          if (typeof id === "string") this.#shown.add(id);
        }
      }
    } catch {
      // In-memory IDs and notification tags still suppress repeated deliveries.
    }
    if (this.#shown.has(notification.id)) return;

    const registration = await navigator.serviceWorker?.getRegistration();
    if (this.#disposed || !this.enabled()) return;
    const options: NotificationOptions = {
      body:
        count > 1
          ? `${notification.message}\n${count - 1} more new ${count === 2 ? "notification" : "notifications"}`
          : notification.message,
      icon: "/icon-192.png",
      tag: `pandan:${this.userId}:${notification.id}`,
      data: {
        type: MESSAGE_TYPE,
        userId: this.userId,
        notificationId: notification.id,
      },
    };
    const title = notification.title || notification.topic_label || "Pandan";
    if (registration?.active) {
      await registration.showNotification(title, options);
    } else {
      // Development and desktop browsers without an active service worker.
      const native = new Notification(title, options);
      this.#native.add(native);
      native.onclose = () => this.#native.delete(native);
      native.onclick = () => {
        native.close();
        if (this.#disposed) return;
        window.focus();
        this.onOpen(notification.id);
      };
    }
    this.#shown.add(notification.id);
    this.#shown = new Set([...this.#shown].slice(-HISTORY_LIMIT));
    try {
      // Store opaque IDs only, never notification content or credentials.
      localStorage.setItem(this.#historyKey, JSON.stringify([...this.#shown]));
    } catch {
      // Delivery succeeded even if recording the ID did not.
    }
  }

  #handleMessage = (event: MessageEvent) => {
    const data = event.data;
    if (
      !this.#disposed &&
      data?.type === MESSAGE_TYPE &&
      data.userId === this.userId &&
      typeof data.notificationId === "string"
    ) {
      this.onOpen(data.notificationId);
    }
  };

  #handleHash = () => {
    const params = new URLSearchParams(window.location.hash.slice(1));
    const id = params.get("notification");
    if (!id || !params.has("notification-user")) return;
    window.history.replaceState(
      window.history.state,
      "",
      window.location.pathname + window.location.search,
    );
    if (!this.#disposed && params.get("notification-user") === this.userId)
      this.onOpen(id);
  };

  dispose() {
    this.#disposed = true;
    navigator.serviceWorker?.removeEventListener(
      "message",
      this.#handleMessage,
    );
    window.removeEventListener("hashchange", this.#handleHash);
    for (const notification of this.#native) notification.close();
    this.#native.clear();
  }
}
