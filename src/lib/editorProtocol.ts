export const EDITOR_OPEN_EVENT = "butler-editor-open";
export const EDITOR_READY_EVENT = "butler-editor-ready";

export interface EditorOpenPayload {
  label: string;
  url: string;
  password: string | null;
}

export interface EditorReadyPayload {
  label: string;
}
