import { useCallback, useEffect, useMemo, useState } from "react";
import { KillSessionDialog } from "./components/KillSessionDialog";
import { SessionCard } from "./components/SessionCard";
import { Workspace } from "./components/Workspace";
import {
  getBackendMode,
  killSession,
  listSessions,
  type BackendMode,
} from "./lib/backend";
import { groupSessions } from "./lib/session";
import type { Session } from "./types";
import "./app.css";

export default function App() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [killTarget, setKillTarget] = useState<Session | null>(null);
  const [killingSessionId, setKillingSessionId] = useState<string | null>(null);
  const [snapshotMs, setSnapshotMs] = useState(Date.now());
  const [nowMs, setNowMs] = useState(Date.now());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [backendMode] = useState<BackendMode>(getBackendMode);

  const refreshSessions = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const nextSessions = await listSessions();
      setSessions(nextSessions);
      const now = Date.now();
      setSnapshotMs(now);
      setNowMs(now);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshSessions();
  }, [refreshSessions]);

  useEffect(() => {
    const timer = window.setInterval(() => setNowMs(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (sessions.length === 0) {
      setSelectedSessionId(null);
      return;
    }

    const selectionStillExists = sessions.some(
      (session) => session.oodSessionId === selectedSessionId,
    );

    if (!selectionStillExists) {
      const preferred =
        sessions.find((session) => session.state === "running") ?? sessions[0];
      setSelectedSessionId(preferred.oodSessionId);
    }
  }, [selectedSessionId, sessions]);

  const groups = useMemo(() => groupSessions(sessions), [sessions]);
  const selectedSession =
    sessions.find((session) => session.oodSessionId === selectedSessionId) ?? null;

  async function confirmKill() {
    if (!killTarget) {
      return;
    }

    setKillingSessionId(killTarget.oodSessionId);
    setError(null);

    try {
      const nextSessions = await killSession(killTarget.oodSessionId);
      setSessions(nextSessions);
      setKillTarget(null);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setKillingSessionId(null);
    }
  }

  const connectionLabel = loading
    ? "Loading sessions"
    : error
      ? "Backend unavailable"
      : backendMode === "tauri"
        ? "Mock cluster service"
        : "Browser preview";

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <header className="sidebar-header">
          <div className="brand">
            <span className="brand__mark" aria-hidden="true">
              B
            </span>
            <span>
              <strong>Butler</strong>
              <small>Code Server manager</small>
            </span>
          </div>
          <button
            className="icon-button"
            type="button"
            aria-label="Refresh sessions"
            title="Refresh sessions"
            onClick={() => void refreshSessions()}
            disabled={loading}
          >
            <RefreshIcon spinning={loading} />
          </button>
        </header>

        {error ? (
          <div className="error-banner" role="alert">
            <strong>Could not refresh sessions.</strong>
            <span>{error}</span>
          </div>
        ) : null}

        <div className="sidebar-summary">
          <span>{sessions.length} active session{sessions.length === 1 ? "" : "s"}</span>
          <span>{groups.length} project{groups.length === 1 ? "" : "s"}</span>
        </div>

        <nav className="project-list" aria-label="Projects and sessions">
          {groups.map((group) => (
            <section className="project-group" key={group.key}>
              <header className="project-group__header">
                <span className="project-group__chevron" aria-hidden="true">
                  <ChevronIcon />
                </span>
                <span className="project-group__identity">
                  <strong>{group.name}</strong>
                  <small>{group.sessions.length} session{group.sessions.length === 1 ? "" : "s"}</small>
                </span>
              </header>

              <div className="project-group__sessions">
                {group.sessions.map((session) => (
                  <SessionCard
                    key={session.oodSessionId}
                    session={session}
                    selected={session.oodSessionId === selectedSessionId}
                    nowMs={nowMs}
                    snapshotMs={snapshotMs}
                    killing={session.oodSessionId === killingSessionId}
                    onSelect={setSelectedSessionId}
                    onKill={setKillTarget}
                  />
                ))}
              </div>
            </section>
          ))}

          {!loading && groups.length === 0 ? (
            <p className="sidebar-empty">No active sessions were returned.</p>
          ) : null}
        </nav>

        <footer className="connection-card">
          <span
            className={`connection-card__dot${error ? " connection-card__dot--error" : ""}`}
            aria-hidden="true"
          />
          <span>
            <strong>{connectionLabel}</strong>
            <small>SSH and Open OnDemand integration comes next.</small>
          </span>
        </footer>
      </aside>

      <Workspace session={selectedSession} nowMs={nowMs} snapshotMs={snapshotMs} />

      <KillSessionDialog
        session={killTarget}
        busy={killTarget?.oodSessionId === killingSessionId}
        onCancel={() => setKillTarget(null)}
        onConfirm={() => void confirmKill()}
      />
    </div>
  );
}

function errorMessage(cause: unknown): string {
  if (cause instanceof Error) {
    return cause.message;
  }

  return String(cause);
}

function RefreshIcon({ spinning }: { spinning: boolean }) {
  return (
    <svg className={spinning ? "spinner" : undefined} aria-hidden="true" viewBox="0 0 24 24">
      <path d="M18.4 5.6A8.9 8.9 0 0 0 12 3a9 9 0 1 0 8.9 10.5h-2.1A7 7 0 1 1 17 7l-3 3h7V3l-2.6 2.6Z" />
    </svg>
  );
}

function ChevronIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="m7 9 5 5 5-5H7Z" />
    </svg>
  );
}
