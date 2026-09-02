import { useEffect } from "react";
import { RuntimePill } from "./Pills";
import { EditorHost } from "./EditorHost";
import { editorWebviews } from "../lib/editorWebviews";
import { getStatusLabel } from "../lib/session";
import type { BackendMode } from "../lib/backend";
import type { Session } from "../types";

interface WorkspaceProps {
  session: Session | null;
  activeSessionIds: readonly string[];
  backendMode: BackendMode;
  nowMs: number;
  snapshotMs: number;
  suspended: boolean;
}

export function Workspace({
  session,
  activeSessionIds,
  backendMode,
  nowMs,
  snapshotMs,
  suspended,
}: WorkspaceProps) {
  useEffect(() => {
    if (backendMode === "tauri") {
      void editorWebviews.prune(activeSessionIds);
    }
  }, [activeSessionIds, backendMode]);

  useEffect(() => {
    if (backendMode === "tauri" && (!session || suspended)) {
      void editorWebviews.hideAll();
    }
  }, [backendMode, session?.oodSessionId, suspended]);

  if (!session) {
    return (
      <main className="workspace workspace--empty">
        <EmptyWorkspace />
      </main>
    );
  }

  return (
    <main className="workspace">
      <header className="workspace-header">
        <div className="workspace-header__identity">
          <span className="workspace-header__project">
            {session.projectName ?? "Unassigned"}
          </span>
          <span className="workspace-header__separator" aria-hidden="true">
            /
          </span>
          <strong>{session.friendlyName}</strong>
        </div>
        <div className="workspace-header__meta">
          <span className={`status status--${session.state}`}>
            <span className="status__dot" aria-hidden="true" />
            {getStatusLabel(session.state)}
          </span>
          <RuntimePill
            session={session}
            nowMs={nowMs}
            snapshotMs={snapshotMs}
            large
          />
        </div>
      </header>

      <section className="editor-stage editor-stage--live">
        <EditorHost
          key={session.oodSessionId}
          session={session}
          backendMode={backendMode}
          suspended={suspended}
        />
      </section>
    </main>
  );
}

function EmptyWorkspace() {
  return (
    <section className="empty-state">
      <span className="empty-state__icon" aria-hidden="true">
        <ButlerGlyph />
      </span>
      <p className="eyebrow">No active sessions</p>
      <h1>Butler is ready.</h1>
      <p>Refresh after Open OnDemand starts a Code Server allocation.</p>
    </section>
  );
}

function ButlerGlyph() {
  return (
    <svg viewBox="0 0 64 64">
      <path d="M17 11h22c8.3 0 14 4.5 14 11.6 0 4.8-2.5 8.3-6.9 10.1C51.8 34.3 55 38 55 43.4 55 51 49 56 39.7 56H17V11Zm11 9v9h10.2c3.1 0 4.8-1.5 4.8-4.5S41.2 20 38.2 20H28Zm0 17v10h11.1c3.8 0 5.9-1.7 5.9-5s-2.1-5-5.9-5H28Z" />
    </svg>
  );
}
