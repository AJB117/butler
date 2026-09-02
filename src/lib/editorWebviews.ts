import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Webview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { closeSession, openSession } from "./backend";
import {
  EDITOR_OPEN_EVENT,
  EDITOR_READY_EVENT,
  type EditorOpenPayload,
  type EditorReadyPayload,
} from "./editorProtocol";

const MAX_CACHED_EDITORS = 5;
const BOOTSTRAP_TIMEOUT_MS = 10_000;

interface EditorEntry {
  sessionId: string;
  webview: Webview;
  lastUsedAt: number;
}

interface ReadyWaiter {
  resolve: () => void;
  reject: (cause: Error) => void;
  timeout: number;
}

export interface EditorBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

class EditorWebviewManager {
  private readonly entries = new Map<string, EditorEntry>();
  private readonly pending = new Map<string, Promise<void>>();
  private readonly readyWaiters = new Map<string, ReadyWaiter>();
  private readyListener: Promise<UnlistenFn> | null = null;
  private desiredSessionId: string | null = null;
  private activeSessionId: string | null = null;

  async activate(sessionId: string, element: HTMLElement): Promise<void> {
    this.desiredSessionId = sessionId;
    await this.hideEditorsExcept(sessionId);

    const cached = this.entries.get(sessionId);
    if (cached) {
      await this.showEntry(cached, measureBounds(element));
      return;
    }

    const opening = this.pending.get(sessionId);
    if (opening) {
      await opening;
      const opened = this.entries.get(sessionId);
      if (opened && this.desiredSessionId === sessionId) {
        await this.showEntry(opened, measureBounds(element));
      }
      return;
    }

    const task = this.createEntry(sessionId, measureBounds(element));
    this.pending.set(sessionId, task);

    try {
      await task;
    } finally {
      this.pending.delete(sessionId);
    }
  }

  async sync(sessionId: string, element: HTMLElement): Promise<void> {
    if (this.activeSessionId !== sessionId) {
      return;
    }

    const entry = this.entries.get(sessionId);
    if (!entry) {
      return;
    }

    await setBounds(entry.webview, measureBounds(element));
  }

  async hideAll(): Promise<void> {
    this.desiredSessionId = null;
    this.activeSessionId = null;
    await Promise.all(
      [...this.entries.values()].map((entry) => ignoreFailure(entry.webview.hide())),
    );
  }

  async prune(activeSessionIds: readonly string[]): Promise<void> {
    const active = new Set(activeSessionIds);
    const stale = [...this.entries.keys()].filter((sessionId) => !active.has(sessionId));
    await Promise.all(stale.map((sessionId) => this.discard(sessionId)));
  }

  async discard(sessionId: string): Promise<void> {
    if (this.desiredSessionId === sessionId) {
      this.desiredSessionId = null;
    }
    if (this.activeSessionId === sessionId) {
      this.activeSessionId = null;
    }

    const opening = this.pending.get(sessionId);
    if (opening) {
      await ignoreFailure(opening);
    }

    const entry = this.entries.get(sessionId);
    if (entry) {
      this.entries.delete(sessionId);
      await ignoreFailure(entry.webview.close());
    }
    await ignoreFailure(closeSession(sessionId));
  }

  private async createEntry(sessionId: string, bounds: EditorBounds): Promise<void> {
    const connection = await openSession(sessionId);
    const label = webviewLabel(sessionId);
    let webview: Webview | null = null;

    try {
      await this.ensureReadyListener();
      const ready = this.waitForReady(label);
      void ready.catch(() => undefined);

      const oldWebview = await Webview.getByLabel(label);
      if (oldWebview) {
        await ignoreFailure(oldWebview.close());
      }

      webview = new Webview(getCurrentWindow(), label, {
        url: "/editor.html",
        ...bounds,
        focus: false,
        dragDropEnabled: false,
        incognito: true,
        dataDirectory: "code-server",
        backgroundColor: "#0f1620",
      });

      await waitForCreated(webview);
      if (this.desiredSessionId !== sessionId) {
        await ignoreFailure(webview.hide());
      }
      await ready;

      const payload: EditorOpenPayload = {
        label,
        url: connection.url,
        password: connection.password,
      };
      await webview.emitTo(label, EDITOR_OPEN_EVENT, payload);
      payload.password = null;
      connection.password = null;

      const entry: EditorEntry = {
        sessionId,
        webview,
        lastUsedAt: Date.now(),
      };
      this.entries.set(sessionId, entry);

      if (this.desiredSessionId === sessionId) {
        await this.showEntry(entry, bounds);
      } else {
        await ignoreFailure(webview.hide());
      }

      await this.enforceCacheLimit();
    } catch (cause) {
      this.cancelReadyWaiter(label);
      if (webview) {
        await ignoreFailure(webview.close());
      }
      await ignoreFailure(closeSession(sessionId));
      throw cause;
    }
  }

  private async showEntry(entry: EditorEntry, bounds: EditorBounds): Promise<void> {
    if (this.desiredSessionId !== entry.sessionId) {
      return;
    }

    await setBounds(entry.webview, bounds);
    await entry.webview.show();
    await entry.webview.setFocus();
    entry.lastUsedAt = Date.now();
    this.activeSessionId = entry.sessionId;
  }

  private async hideEditorsExcept(sessionId: string): Promise<void> {
    this.activeSessionId = null;
    await Promise.all(
      [...this.entries.values()]
        .filter((entry) => entry.sessionId !== sessionId)
        .map((entry) => ignoreFailure(entry.webview.hide())),
    );
  }

  private async enforceCacheLimit(): Promise<void> {
    while (this.entries.size > MAX_CACHED_EDITORS) {
      const candidate = [...this.entries.values()]
        .filter((entry) => entry.sessionId !== this.desiredSessionId)
        .sort((left, right) => left.lastUsedAt - right.lastUsedAt)[0];

      if (!candidate) {
        return;
      }
      await this.discard(candidate.sessionId);
    }
  }

  private async ensureReadyListener(): Promise<void> {
    if (!this.readyListener) {
      this.readyListener = listen<EditorReadyPayload>(EDITOR_READY_EVENT, ({ payload }) => {
        const waiter = this.readyWaiters.get(payload.label);
        if (!waiter) {
          return;
        }

        window.clearTimeout(waiter.timeout);
        this.readyWaiters.delete(payload.label);
        waiter.resolve();
      });
    }

    await this.readyListener;
  }

  private waitForReady(label: string): Promise<void> {
    this.cancelReadyWaiter(label);
    return new Promise((resolve, reject) => {
      const timeout = window.setTimeout(() => {
        this.readyWaiters.delete(label);
        reject(new Error("The Code Server child WebView did not initialize."));
      }, BOOTSTRAP_TIMEOUT_MS);

      this.readyWaiters.set(label, { resolve, reject, timeout });
    });
  }

  private cancelReadyWaiter(label: string): void {
    const waiter = this.readyWaiters.get(label);
    if (!waiter) {
      return;
    }

    window.clearTimeout(waiter.timeout);
    this.readyWaiters.delete(label);
    waiter.reject(new Error("The Code Server child WebView was replaced before it initialized."));
  }
}

export const editorWebviews = new EditorWebviewManager();

function measureBounds(element: HTMLElement): EditorBounds {
  const rect = element.getBoundingClientRect();
  return {
    x: Math.round(rect.left),
    y: Math.round(rect.top),
    width: Math.max(1, Math.round(rect.width)),
    height: Math.max(1, Math.round(rect.height)),
  };
}

async function setBounds(webview: Webview, bounds: EditorBounds): Promise<void> {
  await webview.setPosition(new LogicalPosition(bounds.x, bounds.y));
  await webview.setSize(new LogicalSize(bounds.width, bounds.height));
}

function webviewLabel(sessionId: string): string {
  const safe = sessionId.replace(/[^a-zA-Z0-9_-]/g, "-").slice(0, 56);
  return `editor-${safe || "session"}`;
}

function waitForCreated(webview: Webview): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("Tauri did not create the Code Server child WebView."));
    }, BOOTSTRAP_TIMEOUT_MS);

    void webview.once("tauri://created", () => {
      window.clearTimeout(timeout);
      resolve();
    });
    void webview.once<string>("tauri://error", ({ payload }) => {
      window.clearTimeout(timeout);
      reject(new Error(payload || "Tauri could not create the Code Server child WebView."));
    });
  });
}

async function ignoreFailure(operation: Promise<unknown>): Promise<void> {
  try {
    await operation;
  } catch {
    // Cleanup is best effort. The next refresh reconciles tunnels and sessions.
  }
}
