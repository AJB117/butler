export type SessionState =
  | "pending"
  | "running"
  | "cancelling"
  | "completed"
  | "cancelled"
  | "expired"
  | "unknown";

export interface GpuAllocation {
  model: string | null;
  count: number;
}

export interface HardwareAllocation {
  cpus: number | null;
  memoryBytes: number | null;
  gpus: GpuAllocation[];
  partition: string | null;
}

export interface RuntimeInfo {
  remainingSeconds: number | null;
  timeLimitSeconds: number | null;
}

export interface Session {
  oodSessionId: string;
  jobId: string;
  friendlyName: string;
  projectId: number | null;
  projectName: string | null;
  remotePath: string | null;
  state: SessionState;
  hardware: HardwareAllocation;
  runtime: RuntimeInfo;
}

export interface SessionGroup {
  key: string;
  name: string;
  remotePath: string | null;
  sessions: Session[];
}
