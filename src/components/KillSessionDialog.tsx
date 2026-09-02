import type { Session } from "../types";

interface KillSessionDialogProps {
  session: Session | null;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

export function KillSessionDialog({
  session,
  busy,
  onCancel,
  onConfirm,
}: KillSessionDialogProps) {
  if (!session) {
    return null;
  }

  return (
    <div className="dialog-backdrop" role="presentation">
      <section
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="kill-dialog-title"
      >
        <span className="dialog__icon" aria-hidden="true">
          <WarningIcon />
        </span>
        <div>
          <p className="eyebrow">Remote termination</p>
          <h2 id="kill-dialog-title">Kill {session.friendlyName}?</h2>
          <p className="dialog__copy">
            This terminates scheduler job <strong>{session.jobId}</strong> and
            every process running inside the remote allocation.
          </p>
        </div>
        <div className="dialog__actions">
          <button className="button button--secondary" type="button" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button className="button button--danger" type="button" onClick={onConfirm} disabled={busy}>
            {busy ? "Killing…" : "Kill session"}
          </button>
        </div>
      </section>
    </div>
  );
}

function WarningIcon() {
  return (
    <svg viewBox="0 0 24 24">
      <path d="M12 2 1.8 20h20.4L12 2Zm0 5.3 5.4 9.7H6.6L12 7.3ZM11 10v4h2v-4h-2Zm0 5.5v2h2v-2h-2Z" />
    </svg>
  );
}
