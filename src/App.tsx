import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type DragEvent as ReactDragEvent,
} from "react";
import { KillSessionDialog } from "./components/KillSessionDialog";
import {
  ProjectDialog,
  type ProjectDraft,
} from "./components/ProjectDialog";
import { SessionCard } from "./components/SessionCard";
import { Workspace } from "./components/Workspace";
import {
  getBackendMode,
  killSession,
  listSessions,
  type BackendMode,
} from "./lib/backend";
import {
  applyProjectSnapshot,
  assignSessionProject,
  createProject,
  deleteProject,
  getProjectSnapshot,
  updateProject,
} from "./lib/projects";
import { groupSessions } from "./lib/session";
import type { Project, ProjectSnapshot, Session } from "./types";
import "./app.css";
import "./editor.css";
import "./projects.css";

interface AppError {
  title: string;
  message: string;
}

const EMPTY_PROJECT_SNAPSHOT: ProjectSnapshot = {
  projects: [],
  assignments: {},
};

export default function App() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [projectSnapshot, setProjectSnapshot] = useState<ProjectSnapshot>(
    EMPTY_PROJECT_SNAPSHOT,
  );
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [killTarget, setKillTarget] = useState<Session | null>(null);
  const [projectEditor, setProjectEditor] = useState<Project | "new" | null>(null);
  const [killingSessionId, setKillingSessionId] = useState<string | null>(null);
  const [movingSessionId, setMovingSessionId] = useState<string | null>(null);
  const [draggingSessionId, setDraggingSessionId] = useState<string | null>(null);
  const [dropProjectId, setDropProjectId] = useState<number | null>(null);
  const [projectBusy, setProjectBusy] = useState(false);
  const [projectError, setProjectError] = useState<string | null>(null);
  const [snapshotMs, setSnapshotMs] = useState(Date.now());
  const [nowMs, setNowMs] = useState(Date.now());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<AppError | null>(null);
  const [backendMode] = useState<BackendMode>(getBackendMode);

  const refreshSessions = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const [remoteSessions, nextProjectSnapshot] = await Promise.all([
        listSessions(),
        getProjectSnapshot(),
      ]);
      setProjectSnapshot(nextProjectSnapshot);
      setSessions(applyProjectSnapshot(remoteSessions, nextProjectSnapshot));
      const now = Date.now();
      setSnapshotMs(now);
      setNowMs(now);
    } catch (cause) {
      setError({
        title: "Could not refresh sessions.",
        message: errorMessage(cause),
      });
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

  const groups = useMemo(
    () => groupSessions(sessions, projectSnapshot.projects),
    [projectSnapshot.projects, sessions],
  );
  const activeSessionIds = useMemo(
    () => sessions.map((session) => session.oodSessionId),
    [sessions],
  );
  const selectedSession =
    sessions.find((session) => session.oodSessionId === selectedSessionId) ?? null;

  function adoptProjectSnapshot(nextSnapshot: ProjectSnapshot) {
    setProjectSnapshot(nextSnapshot);
    setSessions((currentSessions) =>
      applyProjectSnapshot(currentSessions, nextSnapshot),
    );
  }

  async function confirmKill() {
    if (!killTarget) {
      return;
    }

    setKillingSessionId(killTarget.oodSessionId);
    setError(null);

    try {
      const [remoteSessions, nextProjectSnapshot] = await Promise.all([
        killSession(killTarget.oodSessionId),
        getProjectSnapshot(),
      ]);
      setProjectSnapshot(nextProjectSnapshot);
      setSessions(applyProjectSnapshot(remoteSessions, nextProjectSnapshot));
      setKillTarget(null);
    } catch (cause) {
      setError({
        title: "Could not kill session.",
        message: errorMessage(cause),
      });
    } finally {
      setKillingSessionId(null);
    }
  }

  async function saveProject(draft: ProjectDraft) {
    if (!projectEditor) {
      return;
    }

    setProjectBusy(true);
    setProjectError(null);
    setError(null);
    try {
      const nextSnapshot =
        projectEditor === "new"
          ? await createProject(draft.name, draft.remotePath)
          : await updateProject(
              projectEditor.id,
              draft.name,
              draft.remotePath,
            );
      adoptProjectSnapshot(nextSnapshot);
      setProjectEditor(null);
    } catch (cause) {
      setProjectError(errorMessage(cause));
    } finally {
      setProjectBusy(false);
    }
  }

  async function removeProject(project: Project) {
    setProjectBusy(true);
    setProjectError(null);
    setError(null);
    try {
      const nextSnapshot = await deleteProject(project.id);
      adoptProjectSnapshot(nextSnapshot);
      setProjectEditor(null);
    } catch (cause) {
      setProjectError(errorMessage(cause));
    } finally {
      setProjectBusy(false);
    }
  }

  async function moveSession(sessionId: string, projectId: number) {
    const session = sessions.find(
      (candidate) => candidate.oodSessionId === sessionId,
    );
    if (!session || session.projectId === projectId || movingSessionId) {
      return;
    }

    setMovingSessionId(sessionId);
    setError(null);
    try {
      const nextSnapshot = await assignSessionProject(sessionId, projectId);
      adoptProjectSnapshot(nextSnapshot);
    } catch (cause) {
      setError({
        title: "Could not move session.",
        message: errorMessage(cause),
      });
    } finally {
      setMovingSessionId(null);
      setDraggingSessionId(null);
      setDropProjectId(null);
    }
  }

  function allowProjectDrop(
    event: ReactDragEvent<HTMLElement>,
    projectId: number,
  ) {
    if (!draggingSessionId) {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    setDropProjectId(projectId);
  }

  function leaveProjectDrop(
    event: ReactDragEvent<HTMLElement>,
    projectId: number,
  ) {
    const relatedTarget = event.relatedTarget;
    if (
      dropProjectId === projectId &&
      (!(relatedTarget instanceof Node) ||
        !event.currentTarget.contains(relatedTarget))
    ) {
      setDropProjectId(null);
    }
  }

  function dropOnProject(
    event: ReactDragEvent<HTMLElement>,
    projectId: number,
  ) {
    event.preventDefault();
    const sessionId =
      event.dataTransfer.getData("application/x-butler-session") ||
      event.dataTransfer.getData("text/plain") ||
      draggingSessionId;
    setDropProjectId(null);
    setDraggingSessionId(null);
    if (sessionId) {
      void moveSession(sessionId, projectId);
    }
  }

  const connectionLabel = loading
    ? "Loading sessions"
    : error
      ? "Action needs attention"
      : backendMode === "tauri"
        ? "OpenSSH + Open OnDemand"
        : "Browser preview";
  const dialogProject =
    projectEditor && projectEditor !== "new" ? projectEditor : null;

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
            <strong>{error.title}</strong>
            <span>{error.message}</span>
          </div>
        ) : null}

        <div className="sidebar-summary">
          <span>
            {sessions.length} active session{sessions.length === 1 ? "" : "s"}
          </span>
          <span>
            {projectSnapshot.projects.length} project
            {projectSnapshot.projects.length === 1 ? "" : "s"}
          </span>
        </div>

        <div className="sidebar-project-actions">
          <button
            className="new-project-button"
            type="button"
            onClick={() => {
              setProjectError(null);
              setProjectEditor("new");
            }}
            disabled={projectBusy}
          >
            <span aria-hidden="true">
              <PlusIcon />
            </span>
            New project
          </button>
          <span>Drag a session onto a project to move it.</span>
        </div>

        <nav className="project-list" aria-label="Projects and sessions">
          {groups.map((group) => {
            const project = projectSnapshot.projects.find(
              (candidate) => candidate.id === group.projectId,
            );
            const dropTarget = dropProjectId === group.projectId;

            return (
              <section
                className={`project-group${dropTarget ? " project-group--drop-target" : ""}`}
                key={group.key}
                onDragEnter={(event: ReactDragEvent<HTMLElement>) =>
                  allowProjectDrop(event, group.projectId)
                }
                onDragOver={(event: ReactDragEvent<HTMLElement>) =>
                  allowProjectDrop(event, group.projectId)
                }
                onDragLeave={(event: ReactDragEvent<HTMLElement>) =>
                  leaveProjectDrop(event, group.projectId)
                }
                onDrop={(event: ReactDragEvent<HTMLElement>) =>
                  dropOnProject(event, group.projectId)
                }
              >
                <header className="project-group__header">
                  <span className="project-group__chevron" aria-hidden="true">
                    <ChevronIcon />
                  </span>
                  <span className="project-group__identity">
                    <strong>{group.name}</strong>
                    <small>
                      {group.sessions.length} session
                      {group.sessions.length === 1 ? "" : "s"}
                    </small>
                  </span>
                  {project ? (
                    <button
                      className="icon-button project-group__edit"
                      type="button"
                      aria-label={`Edit ${group.name}`}
                      title={`Edit ${group.name}`}
                      onClick={() => {
                        setProjectError(null);
                        setProjectEditor(project);
                      }}
                      disabled={projectBusy}
                    >
                      <EditIcon />
                    </button>
                  ) : null}
                </header>

                <div className="project-group__sessions">
                  {group.sessions.map((session) => (
                    <SessionCard
                      key={session.oodSessionId}
                      session={session}
                      projects={projectSnapshot.projects}
                      selected={session.oodSessionId === selectedSessionId}
                      nowMs={nowMs}
                      snapshotMs={snapshotMs}
                      killing={session.oodSessionId === killingSessionId}
                      moving={session.oodSessionId === movingSessionId}
                      dragging={session.oodSessionId === draggingSessionId}
                      onSelect={setSelectedSessionId}
                      onKill={setKillTarget}
                      onMove={(sessionId, projectId) =>
                        void moveSession(sessionId, projectId)
                      }
                      onDragStart={setDraggingSessionId}
                      onDragEnd={() => {
                        setDraggingSessionId(null);
                        setDropProjectId(null);
                      }}
                    />
                  ))}
                  {group.sessions.length === 0 ? (
                    <span className="project-group__empty">
                      {draggingSessionId ? "Drop session here" : "No sessions"}
                    </span>
                  ) : null}
                </div>
              </section>
            );
          })}

          {!loading && groups.length === 0 ? (
            <p className="sidebar-empty">No projects were returned.</p>
          ) : null}
        </nav>

        <footer className="connection-card">
          <span
            className={`connection-card__dot${error ? " connection-card__dot--error" : ""}`}
            aria-hidden="true"
          />
          <span>
            <strong>{connectionLabel}</strong>
            <small>Slurm discovery; editors tunnel on first selection.</small>
          </span>
        </footer>
      </aside>

      <Workspace
        session={selectedSession}
        activeSessionIds={activeSessionIds}
        backendMode={backendMode}
        nowMs={nowMs}
        snapshotMs={snapshotMs}
        suspended={killTarget !== null || projectEditor !== null}
      />

      <KillSessionDialog
        session={killTarget}
        busy={killTarget?.oodSessionId === killingSessionId}
        onCancel={() => setKillTarget(null)}
        onConfirm={() => void confirmKill()}
      />

      <ProjectDialog
        open={projectEditor !== null}
        project={dialogProject}
        busy={projectBusy}
        error={projectError}
        onCancel={() => {
          setProjectError(null);
          setProjectEditor(null);
        }}
        onDelete={(project) => void removeProject(project)}
        onSave={(draft) => void saveProject(draft)}
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
    <svg
      className={spinning ? "spinner" : undefined}
      aria-hidden="true"
      viewBox="0 0 24 24"
    >
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

function PlusIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="M11 5h2v6h6v2h-6v6h-2v-6H5v-2h6V5Z" />
    </svg>
  );
}

function EditIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="m15.7 4.3 4 4L9 19H5v-4L15.7 4.3Zm0 2.8L7 15.8V17h1.2l8.7-8.7-1.2-1.2Z" />
    </svg>
  );
}
