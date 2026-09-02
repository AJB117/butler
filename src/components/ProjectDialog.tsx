import {
  useEffect,
  useState,
  type ChangeEvent,
  type FormEvent,
} from "react";
import type { Project } from "../types";

export interface ProjectDraft {
  name: string;
  remotePath: string | null;
}

interface ProjectDialogProps {
  open: boolean;
  project: Project | null;
  busy: boolean;
  error: string | null;
  onCancel: () => void;
  onDelete: (project: Project) => void;
  onSave: (draft: ProjectDraft) => void;
}

export function ProjectDialog({
  open,
  project,
  busy,
  error,
  onCancel,
  onDelete,
  onSave,
}: ProjectDialogProps) {
  const [name, setName] = useState("");
  const [remotePath, setRemotePath] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);

  useEffect(() => {
    if (!open) {
      return;
    }
    setName(project?.name ?? "");
    setRemotePath(project?.remotePath ?? "");
    setConfirmDelete(false);
  }, [open, project?.id, project?.name, project?.remotePath]);

  if (!open) {
    return null;
  }

  const isNew = project === null;

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onSave({
      name,
      remotePath: remotePath.trim() || null,
    });
  }

  return (
    <div className="dialog-backdrop" role="presentation">
      <form
        className="dialog project-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="project-dialog-title"
        onSubmit={submit}
      >
        <span className="dialog__icon project-dialog__icon" aria-hidden="true">
          <FolderIcon />
        </span>
        <div>
          <p className="eyebrow">Local organization</p>
          <h2 id="project-dialog-title">
            {isNew ? "Create project" : `Edit ${project.name}`}
          </h2>
          <p className="dialog__copy">
            Projects are stored only on this Mac. Moving a session does not move
            remote files or change its Slurm job.
          </p>
        </div>

        <div className="project-dialog__fields">
          {error ? (
            <div className="project-dialog__error" role="alert">
              {error}
            </div>
          ) : null}
          <label>
            <span>Project name</span>
            <input
              autoFocus
              maxLength={80}
              value={name}
              onChange={(event: ChangeEvent<HTMLInputElement>) =>
                setName(event.currentTarget.value)
              }
              placeholder="Research"
              disabled={busy}
            />
          </label>
          <label>
            <span>Default remote folder</span>
            <input
              value={remotePath}
              onChange={(event: ChangeEvent<HTMLInputElement>) =>
                setRemotePath(event.currentTarget.value)
              }
              placeholder="~/projects/research"
              disabled={busy}
            />
          </label>
        </div>

        <div className="dialog__actions project-dialog__actions">
          {project && !project.isDefault ? (
            confirmDelete ? (
              <span className="project-dialog__delete-confirm">
                <span>Move its sessions to the default project?</span>
                <button
                  className="button button--danger"
                  type="button"
                  onClick={() => onDelete(project)}
                  disabled={busy}
                >
                  Delete
                </button>
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => setConfirmDelete(false)}
                  disabled={busy}
                >
                  Keep
                </button>
              </span>
            ) : (
              <button
                className="button button--ghost-danger project-dialog__delete"
                type="button"
                onClick={() => setConfirmDelete(true)}
                disabled={busy}
              >
                Delete project
              </button>
            )
          ) : null}
          <span className="project-dialog__action-spacer" />
          <button
            className="button button--secondary"
            type="button"
            onClick={onCancel}
            disabled={busy}
          >
            Cancel
          </button>
          <button
            className="button button--primary"
            type="submit"
            disabled={busy || name.trim().length === 0}
          >
            {busy ? "Saving…" : isNew ? "Create project" : "Save changes"}
          </button>
        </div>
      </form>
    </div>
  );
}

function FolderIcon() {
  return (
    <svg viewBox="0 0 24 24">
      <path d="M3 5.5C3 4.7 3.7 4 4.5 4h5l2 2h8c.8 0 1.5.7 1.5 1.5v10c0 .8-.7 1.5-1.5 1.5h-15C3.7 19 3 18.3 3 17.5v-12ZM5 8v9h14V8H5Z" />
    </svg>
  );
}
