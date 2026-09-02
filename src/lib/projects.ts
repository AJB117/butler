import { invoke } from "@tauri-apps/api/core";
import type { Project, ProjectSnapshot, Session } from "../types";
import { getBackendMode } from "./backend";

const BROWSER_STORAGE_KEY = "butler.projects.v1";
const DEFAULT_PROJECT: Project = {
  id: 0,
  name: "Unassigned",
  remotePath: null,
  sortOrder: 0,
  isDefault: true,
};

let browserSnapshot = loadBrowserSnapshot();

export async function getProjectSnapshot(): Promise<ProjectSnapshot> {
  if (getBackendMode() === "browser-preview") {
    return cloneSnapshot(browserSnapshot);
  }

  return invoke<ProjectSnapshot>("project_snapshot");
}

export async function createProject(
  name: string,
  remotePath: string | null,
): Promise<ProjectSnapshot> {
  if (getBackendMode() === "browser-preview") {
    const cleanName = validateName(name);
    ensureUniqueName(browserSnapshot.projects, cleanName);
    const id = nextProjectId(browserSnapshot.projects);
    const sortOrder =
      Math.max(
        0,
        ...browserSnapshot.projects.map((project) => project.sortOrder),
      ) + 1;
    browserSnapshot.projects.push({
      id,
      name: cleanName,
      remotePath: normalizeRemotePath(remotePath),
      sortOrder,
      isDefault: false,
    });
    persistBrowserSnapshot();
    return cloneSnapshot(browserSnapshot);
  }

  return invoke<ProjectSnapshot>("create_project", {
    name,
    remotePath,
  });
}

export async function updateProject(
  projectId: number,
  name: string,
  remotePath: string | null,
): Promise<ProjectSnapshot> {
  if (getBackendMode() === "browser-preview") {
    const project = browserSnapshot.projects.find(
      (candidate) => candidate.id === projectId,
    );
    if (!project) {
      throw new Error(`Project ${projectId} no longer exists.`);
    }
    const cleanName = validateName(name);
    ensureUniqueName(browserSnapshot.projects, cleanName, projectId);
    project.name = cleanName;
    project.remotePath = normalizeRemotePath(remotePath);
    persistBrowserSnapshot();
    return cloneSnapshot(browserSnapshot);
  }

  return invoke<ProjectSnapshot>("update_project", {
    projectId,
    name,
    remotePath,
  });
}

export async function deleteProject(projectId: number): Promise<ProjectSnapshot> {
  if (getBackendMode() === "browser-preview") {
    if (projectId === DEFAULT_PROJECT.id) {
      throw new Error("The default project cannot be deleted.");
    }
    if (!browserSnapshot.projects.some((project) => project.id === projectId)) {
      throw new Error(`Project ${projectId} no longer exists.`);
    }
    browserSnapshot.projects = browserSnapshot.projects.filter(
      (project) => project.id !== projectId,
    );
    browserSnapshot.assignments = Object.fromEntries(
      Object.entries(browserSnapshot.assignments).filter(
        ([, assignedProjectId]) => assignedProjectId !== projectId,
      ),
    );
    persistBrowserSnapshot();
    return cloneSnapshot(browserSnapshot);
  }

  return invoke<ProjectSnapshot>("delete_project", { projectId });
}

export async function assignSessionProject(
  sessionId: string,
  projectId: number,
): Promise<ProjectSnapshot> {
  if (getBackendMode() === "browser-preview") {
    if (!browserSnapshot.projects.some((project) => project.id === projectId)) {
      throw new Error(`Project ${projectId} no longer exists.`);
    }
    if (projectId === DEFAULT_PROJECT.id) {
      delete browserSnapshot.assignments[sessionId];
    } else {
      browserSnapshot.assignments[sessionId] = projectId;
    }
    persistBrowserSnapshot();
    return cloneSnapshot(browserSnapshot);
  }

  return invoke<ProjectSnapshot>("assign_session_project", {
    sessionId,
    projectId,
  });
}

export function applyProjectSnapshot(
  sessions: readonly Session[],
  snapshot: ProjectSnapshot,
): Session[] {
  const projects = new Map(
    snapshot.projects.map((project) => [project.id, project]),
  );
  const defaultProject =
    snapshot.projects.find((project) => project.isDefault) ??
    snapshot.projects[0] ??
    DEFAULT_PROJECT;

  return sessions.map((session) => {
    const assignedProjectId =
      snapshot.assignments[session.oodSessionId] ?? defaultProject.id;
    const project = projects.get(assignedProjectId) ?? defaultProject;
    return {
      ...session,
      projectId: project.id,
      projectName: project.name,
      remotePath: project.remotePath,
    };
  });
}

function loadBrowserSnapshot(): ProjectSnapshot {
  try {
    const stored = window.localStorage.getItem(BROWSER_STORAGE_KEY);
    if (!stored) {
      return { projects: [{ ...DEFAULT_PROJECT }], assignments: {} };
    }
    const parsed = JSON.parse(stored) as Partial<ProjectSnapshot>;
    const projects = Array.isArray(parsed.projects)
      ? parsed.projects.filter(isProject).map((project) => ({ ...project }))
      : [];
    if (
      !projects.some(
        (project) =>
          project.isDefault || project.id === DEFAULT_PROJECT.id,
      )
    ) {
      projects.unshift({ ...DEFAULT_PROJECT });
    }
    for (const project of projects) {
      project.isDefault = project.id === DEFAULT_PROJECT.id;
    }
    projects.sort(
      (left, right) => left.sortOrder - right.sortOrder || left.id - right.id,
    );
    const validProjectIds = new Set(projects.map((project) => project.id));
    const assignments = Object.fromEntries(
      Object.entries(parsed.assignments ?? {}).filter(
        ([sessionId, projectId]) =>
          sessionId.length > 0 &&
          typeof projectId === "number" &&
          projectId !== DEFAULT_PROJECT.id &&
          validProjectIds.has(projectId),
      ),
    );
    return { projects, assignments };
  } catch {
    return { projects: [{ ...DEFAULT_PROJECT }], assignments: {} };
  }
}

function persistBrowserSnapshot(): void {
  browserSnapshot.projects.sort(
    (left, right) => left.sortOrder - right.sortOrder || left.id - right.id,
  );
  try {
    window.localStorage.setItem(
      BROWSER_STORAGE_KEY,
      JSON.stringify(browserSnapshot),
    );
  } catch {
    // Browser preview persistence is best effort; Tauri uses the Rust metadata store.
  }
}

function cloneSnapshot(snapshot: ProjectSnapshot): ProjectSnapshot {
  return {
    projects: snapshot.projects.map((project) => ({ ...project })),
    assignments: { ...snapshot.assignments },
  };
}

function nextProjectId(projects: readonly Project[]): number {
  return Math.max(0, ...projects.map((project) => project.id)) + 1;
}

function validateName(name: string): string {
  const cleanName = name.trim();
  if (!cleanName) {
    throw new Error("Project name cannot be empty.");
  }
  if ([...cleanName].length > 80) {
    throw new Error("Project names must be 80 characters or fewer.");
  }
  return cleanName;
}

function ensureUniqueName(
  projects: readonly Project[],
  name: string,
  exceptProjectId?: number,
): void {
  if (
    projects.some(
      (project) =>
        project.id !== exceptProjectId &&
        project.name.toLocaleLowerCase() === name.toLocaleLowerCase(),
    )
  ) {
    throw new Error(`A project named “${name}” already exists.`);
  }
}

function normalizeRemotePath(remotePath: string | null): string | null {
  const value = remotePath?.trim() ?? "";
  if (!value) {
    return null;
  }
  if ([...value].length > 1_024) {
    throw new Error("Remote folders must be 1,024 characters or fewer.");
  }
  return value;
}

function isProject(value: unknown): value is Project {
  if (!value || typeof value !== "object") {
    return false;
  }
  const project = value as Partial<Project>;
  return (
    typeof project.id === "number" &&
    typeof project.name === "string" &&
    (project.remotePath === null || typeof project.remotePath === "string") &&
    typeof project.sortOrder === "number" &&
    typeof project.isDefault === "boolean"
  );
}
