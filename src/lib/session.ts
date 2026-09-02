import type {
  HardwareAllocation,
  Project,
  Session,
  SessionGroup,
  SessionState,
} from "../types";

export type RuntimeTone = "normal" | "warning" | "urgent" | "inactive" | "neutral";

const STATUS_LABELS: Record<SessionState, string> = {
  pending: "Pending",
  running: "Running",
  cancelling: "Cancelling",
  completed: "Completed",
  cancelled: "Cancelled",
  expired: "Expired",
  unknown: "Unknown",
};

export function groupSessions(
  sessions: Session[],
  projects: readonly Project[],
): SessionGroup[] {
  const groups = new Map<number, SessionGroup>();

  for (const project of [...projects].sort(compareProjects)) {
    groups.set(project.id, {
      key: project.id.toString(),
      projectId: project.id,
      name: project.name,
      remotePath: project.remotePath,
      isDefault: project.isDefault,
      sessions: [],
    });
  }

  const defaultProject =
    projects.find((project) => project.isDefault) ?? projects[0] ?? null;

  for (const session of sessions) {
    const projectId = session.projectId ?? defaultProject?.id ?? 0;
    let group = groups.get(projectId);

    if (!group) {
      const project =
        projects.find((candidate) => candidate.id === projectId) ?? defaultProject;
      group = {
        key: projectId.toString(),
        projectId,
        name: project?.name ?? "Unassigned",
        remotePath: project?.remotePath ?? null,
        isDefault: project?.isDefault ?? true,
        sessions: [],
      };
      groups.set(projectId, group);
    }

    group.sessions.push(session);
  }

  return [...groups.values()];
}

export function getStatusLabel(state: SessionState): string {
  return STATUS_LABELS[state];
}

export function getRemainingSeconds(
  session: Session,
  nowMs: number,
  snapshotMs: number,
): number | null {
  const remaining = session.runtime.remainingSeconds;

  if (remaining === null) {
    return null;
  }

  const elapsed = Math.max(0, Math.floor((nowMs - snapshotMs) / 1000));
  return Math.max(0, remaining - elapsed);
}

export function formatRuntime(
  session: Session,
  nowMs: number,
  snapshotMs: number,
): string {
  if (["expired", "completed", "cancelled"].includes(session.state)) {
    return "Expired";
  }

  const remaining = getRemainingSeconds(session, nowMs, snapshotMs);

  if (remaining === null) {
    return session.state === "pending" ? "Awaiting start" : "Unknown";
  }

  if (remaining <= 0) {
    return "Expired";
  }

  const days = Math.floor(remaining / 86_400);
  const hours = Math.floor((remaining % 86_400) / 3_600);
  const minutes = Math.floor((remaining % 3_600) / 60);

  if (days > 0) {
    return `${days}d ${hours}h`;
  }

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }

  return `${Math.max(1, minutes)}m`;
}

export function getRuntimeTone(
  session: Session,
  nowMs: number,
  snapshotMs: number,
): RuntimeTone {
  if (["expired", "completed", "cancelled"].includes(session.state)) {
    return "inactive";
  }

  const remaining = getRemainingSeconds(session, nowMs, snapshotMs);

  if (remaining === null) {
    return "neutral";
  }

  if (remaining <= 0) {
    return "inactive";
  }

  if (remaining <= 60 * 60) {
    return "urgent";
  }

  if (remaining <= 6 * 60 * 60) {
    return "warning";
  }

  return "normal";
}

export function hardwareLabels(hardware: HardwareAllocation): string[] {
  const labels: string[] = [];

  for (const gpu of hardware.gpus) {
    labels.push(`${gpu.model ?? "GPU"} ×${gpu.count}`);
  }

  if (hardware.cpus !== null) {
    labels.push(`${hardware.cpus} CPU`);
  }

  if (hardware.memoryBytes !== null) {
    const gib = hardware.memoryBytes / 1024 ** 3;
    labels.push(`${Number.isInteger(gib) ? gib : gib.toFixed(1)} GB`);
  }

  if (labels.length === 0 && hardware.partition) {
    labels.push(`${hardware.partition} partition`);
  }

  return labels;
}

function compareProjects(left: Project, right: Project): number {
  return left.sortOrder - right.sortOrder || left.id - right.id;
}
