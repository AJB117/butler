import { HardwarePills, RuntimePill } from "./Pills";
import { getStatusLabel } from "../lib/session";
import type { Session } from "../types";

interface SessionCardProps {
  session: Session;
  selected: boolean;
  nowMs: number;
  snapshotMs: number;
  killing: boolean;
  onSelect: (sessionId: string) => void;
  onKill: (session: Session) => void;
}

export function SessionCard({
  session,
  selected,
  nowMs,
  snapshotMs,
  killing,
  onSelect,
  onKill,
}: SessionCardProps) {
  const statusLabel = getStatusLabel(session.state);

  return (
    <article
      className={`session-card${selected ? " session-card--selected" : ""}`}
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
            {statusLabel}
          </span>
          <RuntimePill
            session={session}
            nowMs={nowMs}
            snapshotMs={snapshotMs}
          />
        </span>

        <HardwarePills session={session} compact />
      </button>

      <button
        className="icon-button session-card__kill"
        type="button"
        aria-label={`Kill ${session.friendlyName}`}
        title={`Kill ${session.friendlyName}`}
        disabled={killing || session.state === "cancelling"}
        onClick={() => onKill(session)}
      >
        {killing ? <SpinnerIcon /> : <TrashIcon />}
      </button>
    </article>
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
