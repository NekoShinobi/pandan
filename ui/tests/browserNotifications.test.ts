import { afterEach, beforeEach, test } from "node:test";
import assert from "node:assert/strict";
import {
  BrowserNotifications,
  browserNotificationPermission,
} from "../src/lib/browserNotifications";
import type { NtfyNotification } from "../src/lib/api";

const record: NtfyNotification = {
  id: "message-1",
  remote_id: "remote-1",
  topic_id: "topic-1",
  topic: "updates",
  topic_label: "Updates",
  title: "Build complete",
  message: "The build passed.",
  occurred_at: 123,
  received_at: "2026-09-19",
  priority: 3,
  tags: [],
  actions: [],
  click_url: "https://example.com/",
  seen: false,
};
const originals = new Map<string, PropertyDescriptor | undefined>();
const managers: BrowserNotifications[] = [];
let storage: Map<string, string>;
let shown: { title: string; options: NotificationOptions }[];
let permissionRequests: number;
let opened: string[];
let serviceWorker: EventTarget & { getRegistration: () => Promise<unknown> };
let browserWindow: EventTarget & {
  isSecureContext: boolean;
  Notification?: typeof FakeNotification;
  location: { hash: string; pathname: string; search: string };
  history: {
    state: unknown;
    replaceState: (state: unknown, title: string, url: string) => void;
  };
  focus: () => void;
};

class FakeNotification {
  static permission = "default";
  static instances: FakeNotification[] = [];
  static async requestPermission() {
    permissionRequests++;
    this.permission = "granted";
    return this.permission;
  }
  onclick?: () => void;
  onclose?: () => void;
  closed = false;
  constructor(title: string, options: NotificationOptions) {
    shown.push({ title, options });
    FakeNotification.instances.push(this);
  }
  close() {
    this.closed = true;
    this.onclose?.();
  }
}

function installGlobal(name: string, value: unknown) {
  originals.set(name, Object.getOwnPropertyDescriptor(globalThis, name));
  Object.defineProperty(globalThis, name, { configurable: true, value });
}

function manager(user = "alice") {
  const instance = new BrowserNotifications(user, (id) => opened.push(id));
  managers.push(instance);
  return instance;
}

beforeEach(() => {
  storage = new Map();
  shown = [];
  opened = [];
  permissionRequests = 0;
  FakeNotification.permission = "default";
  FakeNotification.instances = [];
  browserWindow = Object.assign(new EventTarget(), {
    isSecureContext: true,
    Notification: FakeNotification,
    location: { hash: "", pathname: "/", search: "" },
    history: {
      state: {},
      replaceState: () => {
        browserWindow.location.hash = "";
      },
    },
    focus: () => {},
  });
  serviceWorker = Object.assign(new EventTarget(), {
    getRegistration: async (): Promise<unknown> => ({
      active: {},
      showNotification: async (title: string, options: NotificationOptions) => {
        shown.push({ title, options });
      },
    }),
  });
  let queue = Promise.resolve();
  installGlobal("window", browserWindow);
  installGlobal("Notification", FakeNotification);
  installGlobal("localStorage", {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
  });
  installGlobal("navigator", {
    serviceWorker,
    locks: {
      request: (_key: string, callback: () => Promise<void>) => {
        const result = queue.then(callback);
        queue = result.catch(() => {});
        return result;
      },
    },
  });
});

afterEach(() => {
  for (const instance of managers.splice(0)) instance.dispose();
  for (const [name, descriptor] of originals) {
    if (descriptor) Object.defineProperty(globalThis, name, descriptor);
    else Reflect.deleteProperty(globalThis, name);
  }
  originals.clear();
});

test("delivery never prompts; explicit enable requests permission and persists per account", async () => {
  const alice = manager();
  await alice.show(record);
  assert.equal(permissionRequests, 0);
  assert.equal(shown.length, 0);
  const enabling = alice.enable();
  assert.equal(permissionRequests, 1);
  assert.equal(await enabling, true);
  await alice.show(record);
  assert.equal(shown.length, 1);
  assert.equal(manager().enabled(), true);
  assert.equal(manager("bob").enabled(), false);
  alice.disable();
  await alice.show({ ...record, id: "message-2" });
  assert.equal(shown.length, 1);
});

test("denied, unsupported and revoked permission suppress alerts without throwing", async () => {
  const alice = manager();
  FakeNotification.permission = "denied";
  assert.equal(await alice.enable(), false);
  assert.equal(permissionRequests, 0);
  FakeNotification.permission = "granted";
  await alice.enable();
  FakeNotification.permission = "denied";
  await alice.show(record);
  browserWindow.isSecureContext = false;
  assert.equal(browserNotificationPermission(), "unsupported");
  assert.equal(await alice.enable(), false);
  browserWindow.isSecureContext = true;
  delete browserWindow.Notification;
  assert.equal(browserNotificationPermission(), "unsupported");
  assert.equal(shown.length, 0);
});

test("concurrent tabs and a remount announce a delivery only once", async () => {
  const first = manager();
  const second = manager();
  await first.enable();
  await Promise.all([first.show(record), second.show(record)]);
  await manager().show(record);
  assert.equal(shown.length, 1);
  assert.deepEqual(shown[0].options.data, {
    type: "PANDAN_OPEN_NOTIFICATION",
    userId: "alice",
    notificationId: record.id,
  });
  const bob = manager("bob");
  await bob.enable();
  await bob.show(record);
  assert.equal(shown.length, 2);
  assert.notEqual(shown[0].options.tag, shown[1].options.tag);
  assert.ok(
    ![...storage.values()].some((value) => value.includes(record.message)),
  );
});

test("a failed OS delivery stays retryable", async () => {
  const alice = manager();
  await alice.enable();
  const registration = serviceWorker.getRegistration;
  serviceWorker.getRegistration = async () => ({
    active: {},
    showNotification: async () => {
      throw new Error("OS blocked");
    },
  });
  await assert.doesNotReject(alice.show(record));
  serviceWorker.getRegistration = registration;
  await alice.show(record);
  assert.equal(shown.length, 1);
});

test("a pending delivery cannot notify after the authenticated shell is destroyed", async () => {
  const alice = manager();
  await alice.enable();
  let resolveRegistration!: (value: unknown) => void;
  let started!: () => void;
  const registrationStarted = new Promise<void>((resolve) => {
    started = resolve;
  });
  serviceWorker.getRegistration = () => {
    started();
    return new Promise((resolve) => {
      resolveRegistration = resolve;
    });
  };
  const pending = alice.show(record);
  await registrationStarted;
  alice.dispose();
  resolveRegistration({
    active: {},
    showNotification: async () => assert.fail("late alert"),
  });
  await pending;
  assert.equal(shown.length, 0);
});

test("click messages only open the matching account and listeners are removed", () => {
  const alice = manager();
  const click = (userId: string) =>
    serviceWorker.dispatchEvent(
      new MessageEvent("message", {
        data: {
          type: "PANDAN_OPEN_NOTIFICATION",
          userId,
          notificationId: record.id,
        },
      }),
    );
  click("bob");
  assert.deepEqual(opened, []);
  click("alice");
  assert.deepEqual(opened, [record.id]);
  alice.dispose();
  click("alice");
  assert.equal(opened.length, 1);
});

test("fresh-window click URLs are consumed only for their account", () => {
  browserWindow.location.hash =
    "#notification=message-1&notification-user=alice";
  manager();
  assert.deepEqual(opened, [record.id]);
  assert.equal(browserWindow.location.hash, "");
  browserWindow.location.hash = "#notification=private&notification-user=bob";
  browserWindow.dispatchEvent(new Event("hashchange"));
  assert.deepEqual(opened, [record.id]);
  assert.equal(browserWindow.location.hash, "");
});

test("desktop fallback opens Pandan and closes its notifications on teardown", async () => {
  serviceWorker.getRegistration = async () => undefined;
  const alice = manager();
  await alice.enable();
  await alice.show(record, 3);
  assert.match(shown[0].options.body!, /2 more new notifications/);
  FakeNotification.instances[0].onclick?.();
  assert.deepEqual(opened, [record.id]);
  await alice.show({ ...record, id: "message-2" });
  alice.dispose();
  assert.ok(
    FakeNotification.instances.every((notification) => notification.closed),
  );
});
