import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";

// Exercise the real worker handler without registering a worker or changing caches.
const source = readFileSync(
  new URL("../src/service-worker.ts", import.meta.url),
  "utf8",
).replace(
  'import { base, build, files, version } from "$service-worker";',
  'const base = "", build = [], files = [], version = "test";',
);
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    target: ts.ScriptTarget.ESNext,
    module: ts.ModuleKind.None,
  },
}).outputText;

const data = {
  type: "PANDAN_OPEN_NOTIFICATION",
  userId: "alice",
  notificationId: "message-1",
};

function setup(clients: unknown[]) {
  let click: (event: unknown) => void;
  let closed = false;
  const windows: string[] = [];
  runInNewContext(compiled, {
    URL,
    URLSearchParams,
    self: {
      location: { origin: "https://pandan.example" },
      addEventListener: (name: string, handler: (event: unknown) => void) => {
        if (name === "notificationclick") click = handler;
      },
      clients: {
        matchAll: async () => clients,
        openWindow: async (url: string) => {
          windows.push(url);
        },
      },
    },
  });
  return {
    windows,
    get closed() {
      return closed;
    },
    async click(payload: unknown = data) {
      let pending = Promise.resolve();
      click({
        notification: {
          data: payload,
          close: () => {
            closed = true;
          },
        },
        waitUntil: (promise: Promise<void>) => {
          pending = promise;
        },
      });
      await pending;
    },
  };
}

test("worker focuses an existing app and forwards the account-qualified notification without reloading", async () => {
  const messages: unknown[] = [];
  let focused = false;
  const worker = setup([
    {
      url: "https://pandan.example/",
      focused: true,
      postMessage: (message: unknown) => messages.push(message),
      focus: async () => {
        focused = true;
      },
    },
  ]);
  await worker.click();
  assert.equal(worker.closed, true);
  assert.equal(focused, true);
  assert.deepEqual(messages, [data]);
  assert.deepEqual(worker.windows, []);
});

test("worker opens only Pandan when no app window exists, ignoring remote click URLs", async () => {
  const worker = setup([{ url: "https://other.example/", focused: true }]);
  await worker.click({ ...data, url: "https://evil.example/" });
  const destination = new URL(worker.windows[0]);
  assert.equal(destination.origin, "https://pandan.example");
  assert.equal(destination.pathname, "/");
  const params = new URLSearchParams(destination.hash.slice(1));
  assert.equal(params.get("notification"), "message-1");
  assert.equal(params.get("notification-user"), "alice");
});

test("worker ignores malformed and unrelated notification data", async () => {
  const worker = setup([]);
  await worker.click(null);
  await worker.click({ ...data, type: "unrelated" });
  await worker.click({ ...data, userId: 123 });
  assert.deepEqual(worker.windows, []);
});
