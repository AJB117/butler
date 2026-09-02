import { HardwarePills, RuntimePill } from "./Pills";
import { getStatusLabel } from "../lib/session";
import type { Session } from "../types";

interface WorkspaceProps {
  session: Session | null;
  nowMs: number;
  snapshotMs: number;
}

export function Workspace({ session, nowMs, snapshotMs }: WorkspaceProps) {
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

      <section className="editor-stage">
        <aside className="editor-rail" aria-hidden="true">
          <RailIcon kind="files" />
          <RailIcon kind="search" />
          <RailIcon kind="source" />
          <span className="editor-rail__spacer" />
          <RailIcon kind="settings" />
        </aside>

        <div className="editor-surface">
          <div className="editor-tabbar">
            <span className="editor-tab editor-tab--active">
              <span className="editor-tab__dot" />
              Butler session
            </span>
          </div>

          <div className="editor-placeholder">
            <div className="editor-placeholder__mark" aria-hidden="true">
              <ButlerGlyph />
            </div>
            <p className="eyebrow">Session selected</p>
            <h1>{session.friendlyName}</h1>
            <p className="editor-placeholder__copy">
              This region is reserved for the Tauri child WebView. The next
              cluster-backed slice will establish an SSH tunnel and place the
              live Code Server editor here without using an iframe.
            </p>

            <HardwarePills session={session} />

            <dl className="session-facts">
              <Fact label="Remote folder" value={session.remotePath ?? "Not assigned"} />
              <Fact label="Scheduler job" value={session.jobId} />
              <Fact label="OOD session" value={shortId(session.oodSessionId)} />
            </dl>

            <div className="connection-pipeline" aria-label="Session opening pipeline">
              <PipelineStep label="Session selected" complete />
              <PipelineArrow />
              <PipelineStep label="SSH tunnel" />
              <PipelineArrow />
              <PipelineStep label="Child WebView" />
            </div>
          </div>

          <footer className="editor-statusbar">
            <span>Butler prototype</span>
            <span>{session.remotePath ?? "No remote folder"}</span>
            <span>Job {session.jobId}</span>
          </footer>
        </div>
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

function Fact({ label, value }: { label: string; value: string }) {
  return (
    <div className="session-facts__item">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function PipelineStep({ label, complete = false }: { label: string; complete?: boolean }) {
  return (
    <span className={`pipeline-step${complete ? " pipeline-step--complete" : ""}`}>
      <span className="pipeline-step__dot" aria-hidden="true" />
      {label}
    </span>
  );
}

function PipelineArrow() {
  return (
    <svg className="pipeline-arrow" aria-hidden="true" viewBox="0 0 24 24">
      <path d="M5 11h11.2l-3.6-3.6L14 6l6 6-6 6-1.4-1.4 3.6-3.6H5v-2Z" />
    </svg>
  );
}

function shortId(id: string): string {
  return `${id.slice(0, 8)}…${id.slice(-4)}`;
}

function ButlerGlyph() {
  return (
    <svg viewBox="0 0 64 64">
      <path d="M17 11h22c8.3 0 14 4.5 14 11.6 0 4.8-2.5 8.3-6.9 10.1C51.8 34.3 55 38 55 43.4 55 51 49 56 39.7 56H17V11Zm11 9v9h10.2c3.1 0 4.8-1.5 4.8-4.5S41.2 20 38.2 20H28Zm0 17v10h11.1c3.8 0 5.9-1.7 5.9-5s-2.1-5-5.9-5H28Z" />
    </svg>
  );
}

function RailIcon({ kind }: { kind: "files" | "search" | "source" | "settings" }) {
  const paths = {
    files: "M5 3h9l5 5v13H5V3Zm2 2v14h10V9h-4V5H7Zm8 .8V7h1.2L15 5.8Z",
    search: "M10.5 4a6.5 6.5 0 1 0 4 11.6L20 21l1-1-5.5-5.5A6.5 6.5 0 0 0 10.5 4Zm0 2a4.5 4.5 0 1 1 0 9 4.5 4.5 0 0 1 0-9Z",
    source: "M6 3a3 3 0 1 0 2 5.2V10a5 5 0 0 0 5 5h3v1.8a3 3 0 1 0 2 0V14h-5a3 3 0 0 1-3-3V8.2A3 3 0 0 0 6 3Zm0 2a1 1 0 1 1 0 2 1 1 0 0 1 0-2Zm11 14a1 1 0 1 1 0 2 1 1 0 0 1 0-2Z",
    settings: "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Zm0 2a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3Zm8.7 2.5.1-1-.1-1-2.1-.8a7 7 0 0 0-.6-1.4l.9-2-1.4-1.4-2 .9a7 7 0 0 0-1.4-.6L13 3.5h-2l-.8 2.1a7 7 0 0 0-1.4.6l-2-.9-1.4 1.4.9 2a7 7 0 0 0-.6 1.4l-2.1.8-.1 1 .1 1 2.1.8a7 7 0 0 0 .6 1.4l-.9 2 1.4 1.4 2-.9a7 7 0 0 0 1.4.6l.8 2.1h2l.8-2.1a7 7 0 0 0 1.4-.6l2 .9 1.4-1.4-.9-2a7 7 0 0 0 .6-1.4l2.4-.7Z",
  };

  return (
    <span className="editor-rail__icon">
      <svg viewBox="0 0 24 24">
        <path d={paths[kind]} />
      </svg>
    </span>
  );
}
