import type { Session } from "../types";
import {
  formatRuntime,
  getRuntimeTone,
  hardwareLabels,
} from "../lib/session";

interface HardwarePillsProps {
  session: Session;
  compact?: boolean;
}

export function HardwarePills({ session, compact = false }: HardwarePillsProps) {
  const labels = hardwareLabels(session.hardware);

  return (
    <div className={`pill-row${compact ? " pill-row--compact" : ""}`}>
      {labels.map((label) => (
        <span className="pill pill--hardware" key={label}>
          {label}
        </span>
      ))}
    </div>
  );
}

interface RuntimePillProps {
  session: Session;
  nowMs: number;
  snapshotMs: number;
  large?: boolean;
}

export function RuntimePill({
  session,
  nowMs,
  snapshotMs,
  large = false,
}: RuntimePillProps) {
  const tone = getRuntimeTone(session, nowMs, snapshotMs);

  return (
    <span
      className={`pill pill--runtime pill--${tone}${large ? " pill--large" : ""}`}
    >
      {formatRuntime(session, nowMs, snapshotMs)}
    </span>
  );
}
