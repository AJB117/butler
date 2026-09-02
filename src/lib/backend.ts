import { invoke } from "@tauri-apps/api/core";
import type {
  BackendStatus,
  OpenSessionResult,
  Session,
} from "../types";

const GIB = 1024 ** 3;

const initialDemoSessions: Session[] = [
  {
    oodSessionId: "03a8efa7-da5f-436f-919f-617b948c1358",
    jobId: "12345678",
    friendlyName: "Training",
    projectId: 1,
    projectName: "Research",
    remotePath: "~/projects/research",
    state: "running",
    hardware: {
      cpus: 8,
      memoryBytes: 64 * GIB,
      gpus: [{ model: "A100", count: 1 }],
      partition: "gpu",
    },
    runtime: {
      remainingSeconds: 2 * 86_400 + 11 * 3_600,
      timeLimitSeconds: 3 * 86_400,
    },
  },
  {
    oodSessionId: "5fa6ed27-f89d-46ca-a6b7-9e21d324c48f",
    jobId: "12345681",
    friendlyName: "Evaluation",
    projectId: 1,
    projectName: "Research",
    remotePath: "~/projects/research/evaluation",
    state: "pending",
    hardware: {
      cpus: 4,
      memoryBytes: 32 * GIB,
      gpus: [{ model: null, count: 1 }],
      partition: "gpu",
    },
    runtime: {
      remainingSeconds: null,
      timeLimitSeconds: 86_400,
    },
  },
  {
    oodSessionId: "f56f937c-a385-4d32-a583-c12767077017",
    jobId: "12345692",
    friendlyName: "Homework 3",
    projectId: 2,
    projectName: "CS 4501",
    remotePath: "~/courses/cs4501/homework-3",
    state: "running",
    hardware: {
      cpus: 8,
      memoryBytes: 32 * GIB,
      gpus: [],
      partition: "standard",
    },
    runtime: {
      remainingSeconds: 18 * 3_600 + 42 * 60,
      timeLimitSeconds: 86_400,
    },
  },
  {
    oodSessionId: "b8e40aaf-27b8-42c6-8918-4296d416dbab",
    jobId: "12345703",
    friendlyName: "Scratch",
    projectId: 3,
    projectName: "Misc",
    remotePath: "~/scratch",
    state: "running",
    hardware: {
      cpus: 4,
      memoryBytes: 16 * GIB,
      gpus: [],
      partition: "standard",
    },
    runtime: {
      remainingSeconds: 37 * 60,
      timeLimitSeconds: 4 * 3_600,
    },
  },
];

let browserSessions = initialDemoSessions.map(cloneSession);

export type BackendMode = "tauri" | "browser-preview";

export function getBackendMode(): BackendMode {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
    ? "tauri"
    : "browser-preview";
}

export async function getBackendStatus(): Promise<BackendStatus> {
  if (getBackendMode() === "browser-preview") {
    return {
      configured: true,
      connected: true,
      configPath: "Browser preview",
      sshTarget: null,
      controlPath: null,
      activeTunnels: 0,
      message: null,
    };
  }

  return invoke<BackendStatus>("backend_status");
}

export async function listSessions(): Promise<Session[]> {
  if (getBackendMode() === "browser-preview") {
    return browserSessions.map(cloneSession);
  }

  return invoke<Session[]>("list_sessions");
}

export async function openSession(sessionId: string): Promise<OpenSessionResult> {
  if (getBackendMode() === "browser-preview") {
    return {
      sessionId,
      localPort: 8080,
      url: "http://127.0.0.1:8080/",
      remoteHost: "preview-compute-node",
      remotePort: 3450,
      password: null,
    };
  }

  return invoke<OpenSessionResult>("open_session", { sessionId });
}

export async function closeSession(sessionId: string): Promise<void> {
  if (getBackendMode() === "browser-preview") {
    return;
  }

  await invoke("close_session", { sessionId });
}

export async function killSession(sessionId: string): Promise<Session[]> {
  if (getBackendMode() === "browser-preview") {
    browserSessions = browserSessions.filter(
      (session) => session.oodSessionId !== sessionId,
    );
    return browserSessions.map(cloneSession);
  }

  return invoke<Session[]>("kill_session", { sessionId });
}

function cloneSession(session: Session): Session {
  return {
    ...session,
    hardware: {
      ...session.hardware,
      gpus: session.hardware.gpus.map((gpu) => ({ ...gpu })),
    },
    runtime: { ...session.runtime },
  };
}
