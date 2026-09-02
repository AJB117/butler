import { useEffect, useRef, useState } from "react";
import { editorWebviews } from "../lib/editorWebviews";
import type { BackendMode } from "../lib/backend";
import type { Session } from "../types";

interface EditorHostProps {
  session: Session;
  backendMode: BackendMode;
  suspended: boolean;
}

type HostState = "opening" | "ready" | "error" | "unavailable" | "preview";

export function EditorHost({ session, backendMode, suspended }: EditorHostProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const [hostState, setHostState] = useState<HostState>("opening");
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) {
      return;
    }

    if (backendMode === "browser-preview") {
      setHostState("preview");
      return;
    }

    if (suspended) {
      void editorWebviews.hideAll();
      return;
    }

    if (session.state !== "running") {
      setHostState("unavailable");
      void editorWebviews.hideAll();
      return;
    }

    let cancelled = false;
    setHostState("opening");
    setError(null);

    void editorWebviews
      .activate(session.oodSessionId, host)
      .then(() => {
        if (!cancelled) {
          setHostState("ready");
        }
      })
      .catch((cause) => {
        if (!cancelled) {
          setHostState("error");
          setError(errorMessage(cause));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [attempt, backendMode, session.oodSessionId, session.state, suspended]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host || backendMode !== "tauri" || session.state !== "running") {
      return;
    }

    const sync = () => {
      void editorWebviews.sync(session.oodSessionId, host);
    };
    const observer = new ResizeObserver(sync);
    observer.observe(host);
    window.addEventListener("resize", sync);
    sync();

    return () => {
      observer.disconnect();
      window.removeEventListener("resize", sync);
    };
  }, [backendMode, session.oodSessionId, session.state]);

  return (
    <div className="editor-webview-host" ref={hostRef}>
      {hostState === "opening" ? (
        <HostMessage busy title="Opening Code Server">
          Butler is reading the session credential and establishing the SSH tunnel.
        </HostMessage>
      ) : null}

      {hostState === "error" ? (
        <HostMessage title="Could not open Code Server" tone="error">
          <span>{error}</span>
          <button className="button button--secondary" type="button" onClick={() => setAttempt((value) => value + 1)}>
            Retry
          </button>
        </HostMessage>
      ) : null}

      {hostState === "unavailable" ? (
        <HostMessage title="Session is not running yet">
          Butler will be able to open this editor after Slurm reports the allocation as running.
        </HostMessage>
      ) : null}

      {hostState === "preview" ? (
        <HostMessage title="Desktop app required">
          Child WebViews and SSH tunnels are available in the Tauri desktop app, not the browser preview.
        </HostMessage>
      ) : null}

      {hostState === "ready" ? <span className="visually-hidden">Code Server is open.</span> : null}
    </div>
  );
}

function HostMessage({
  busy = false,
  children,
  title,
  tone = "normal",
}: {
  busy?: boolean;
  children: React.ReactNode;
  title: string;
  tone?: "normal" | "error";
}) {
  return (
    <div className={`editor-host-message editor-host-message--${tone}`}>
      {busy ? <span className="editor-host-spinner" aria-hidden="true" /> : null}
      <strong>{title}</strong>
      <p>{children}</p>
    </div>
  );
}

function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}
