import {
  useEffect,
  useRef,
  useState,
  type DragEvent as ReactDragEvent,
} from "react";
import { HardwarePills, RuntimePill } from "./Pills";
import { getStatusLabel } from "../lib/session";
import type { Project, Session } from "../types";

interface SessionCardProps {
  session: Session;
  projects: readonly Project[];
  selected: boolean;
  nowMs: number;
  snapshotMs: number;
  killing: boolean;
  moving: boolean;
  dragging: boolean;
  onSelect: (sessionId: string) => void;
  onKill: (session: Session) => void;
  onMove: (sessionId: string, projectId: number) => void;
  onDragStart: (sessionId: string) => void;
  onDragEnd: () => void;
}

export function SessionCard({
  session,
  projects,
  selected,
  nowMs,
  snapshotMs,
  killing,
  moving,
  dragging,
  onSelect,
  onKill,
  onMove,
  onDragStart,
  onDragEnd,
}: SessionCardProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const statusLabel = getStatusLabel(session.state);

  useEffect(() => {
    if (!menuOpen) {
      return;
    }

    const close = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) {
        setMenuOpen(false);
      }
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setMenuOpen(false);
      }
    };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [menuOpen]);

  function startDrag(event: ReactDragEvent<HTMLElement>) {
    if (killing || moving) {
      event.preventDefault();
      return;
    }
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData(
      "application/x-butler-session",
      session.oodSessionId,
    );
    event.dataTransfer.setData("text/plain", session.oodSessionId);
    onDragStart(session.oodSessionId);
  }

  return (
    <article
      className={[
        "session-card",
        selected ? "session-card--selected" : "",
        dragging ? "session-card--dragging" : "",
        menuOpen ? "session-card--menu-open" : "",
      ]
        .filter(Boolean)
        .join(" ")}
      draggable={!killing && !moving}
      onDragStart={startDrag}
      onDragEnd={onDragEnd}
    >
      <button
        className="session-card__select"
        type="button"
        aria-pressed={selected}
        onClick={() => onSelect(session.oodSessionId)}
      >
        <span className="session-card__heading">
          <span className="session-card__name">{session.friendlyName}</span>
          <span className="session-card__path">
            {session.remotePath ?? "No folder assigned"}
          </span>
        </span>

        <span className="session-card__status-row">
          <span className={`status status--${session.state}`}>
            <span className="status__dot" aria-hidden="true" />
            {moving ? "Moving…" : statusLabel}
          </span>
          <RuntimePill
            session={session}
            nowMs={nowMs}
            snapshotMs={snapshotMs}
          />
        </span>

        <HardwarePills session={session} compact />
      </button>

      <div className="session-card__menu" ref={menuRef}>
        <button
          className="icon-button session-card__more"
          type="button"
          aria-label={`Move ${session.friendlyName} to another project`}
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          title="Move to project"
          draggable={false}
          onDragStart={(event: ReactDragEvent<HTMLButtonElement>) =>
            event.preventDefault()
          }
          onClick={() => setMenuOpen((open) => !open)}
          disabled={moving}
        >
          <MoreIcon />
        </button>

        {menuOpen ? (
          <div className="session-move-menu" role="menu">
            <span className="session-move-menu__label">Move to project</span>
            {projects.map((project) => {
              const current = project.id === session.projectId;
              return (
                <button
                  className="session-move-menu__item"
                  type="button"
                  role="menuitemradio"
                  aria-checked={current}
                  key={project.id}
                  disabled={current || moving}
                  onClick={() => {
                    setMenuOpen(false);
                    onMove(session.oodSessionId, project.id);
                  }}
                >
                  <span className="session-move-menu__check" aria-hidden="true">
                    {current ? "✓" : ""}
                  </span>
                  <span>
                    <strong>{project.name}</strong>
                    {project.remotePath ? (
                      <small>{project.remotePath}</small>
                    ) : null}
                  </span>
                </button>
              );
            })}
          </div>
        ) : null}
      </div>

      <button
        className="icon-button session-card__kill"
        type="button"
        aria-label={`Kill ${session.friendlyName}`}
        title={`Kill ${session.friendlyName}`}
        disabled={killing || moving || session.state === "cancelling"}
        draggable={false}
        onDragStart={(event: ReactDragEvent<HTMLButtonElement>) =>
          event.preventDefault()
        }
        onClick={() => onKill(session)}
      >
        {killing ? <SpinnerIcon /> : <TrashIcon />}
      </button>
    </article>
  );
}

function MoreIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="M5 10a2 2 0 1 1 0 4 2 2 0 0 1 0-4Zm7 0a2 2 0 1 1 0 4 2 2 0 0 1 0-4Zm7 0a2 2 0 1 1 0 4 2 2 0 0 1 0-4Z" />
    </svg>
  );
}

function TrashIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="M9 3h6l1 2h4v2H4V5h4l1-2Zm-2 6h10l-.7 11H7.7L7 9Zm3 2v7h2v-7h-2Zm4 0v7h2v-7h-2Z" />
    </svg>
  );
}

function SpinnerIcon() {
  return (
    <svg className="spinner" aria-hidden="true" viewBox="0 0 24 24">
      <path d="M12 4a8 8 0 1 1-7.4 5H2.5A10 10 0 1 0 12 2v2Z" />
    </svg>
  );
}
