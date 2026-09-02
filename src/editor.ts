import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  EDITOR_OPEN_EVENT,
  EDITOR_READY_EVENT,
  type EditorOpenPayload,
  type EditorReadyPayload,
} from "./lib/editorProtocol";

void bootstrap().catch(showError);

async function bootstrap(): Promise<void> {
  const currentWebview = getCurrentWebview();

  await listen<EditorOpenPayload>(EDITOR_OPEN_EVENT, ({ payload }) => {
    if (payload.label !== currentWebview.label) {
      return;
    }

    try {
      openEditor(payload);
    } catch (cause) {
      showError(cause);
    }
  });

  const ready: EditorReadyPayload = { label: currentWebview.label };
  await emitTo("main", EDITOR_READY_EVENT, ready);
}

function openEditor(payload: EditorOpenPayload): void {
  const target = parseLoopbackUrl(payload.url);
  setStatus(payload.password ? "Signing in to Code Server…" : "Loading Code Server…");

  if (!payload.password) {
    window.location.replace(target.href);
    return;
  }

  const login = new URL(target.href);
  const basePath = login.pathname.replace(/\/+$/, "");
  login.pathname = basePath.endsWith("/login")
    ? basePath
    : `${basePath}/login`.replace(/\/{2,}/g, "/");
  login.search = "?to=";
  login.hash = "";

  const form = document.createElement("form");
  form.method = "post";
  form.action = login.href;
  form.hidden = true;
  form.autocomplete = "off";
  appendField(form, "password", payload.password);
  appendField(form, "base", target.pathname || "/");
  appendField(form, "href", target.href);
  document.body.append(form);
  form.submit();
}

function parseLoopbackUrl(value: string): URL {
  const url = new URL(value);
  const host = url.hostname.toLowerCase();

  if (!["127.0.0.1", "localhost", "::1"].includes(host)) {
    throw new Error("Butler refused to open a non-loopback editor URL.");
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error("Butler received an unsupported editor URL protocol.");
  }

  return url;
}

function appendField(form: HTMLFormElement, name: string, value: string): void {
  const input = document.createElement("input");
  input.type = "hidden";
  input.name = name;
  input.value = value;
  form.append(input);
}

function setStatus(message: string): void {
  const status = document.getElementById("status");
  if (status) {
    status.textContent = message;
  }
}

function showError(cause: unknown): void {
  const status = document.getElementById("status");
  const message = cause instanceof Error ? cause.message : String(cause);
  if (status) {
    status.classList.add("error");
    status.textContent = message;
  }
}
