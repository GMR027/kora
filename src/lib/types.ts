// DTOs espejo de los structs de Rust (serde rename_all = "camelCase").
// Si cambias un struct en src-tauri/src, actualiza también este archivo.

export type BookFormat = "epub" | "pdf";

export interface StorageLayout {
  root: string;
  db: string;
  assets: string;
  exports: string;
  trash: string;
  logs: string;
}

export interface AppInfo {
  name: string;
  version: string;
  identifier: string;
  platform: string;
  arch: string;
  storage: StorageLayout;
}

export interface BookSummary {
  id: string;
  title: string;
  author: string | null;
  format: BookFormat;
  coverPath: string | null;
  sizeBytes: number;
  addedAt: string;
  progress: number;
}

export interface Category {
  id: string;
  name: string;
  color: string | null;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
  ownerAccountId: string;
  originShareId: string | null;
}

export interface Locator {
  section: number;
  offset: number;
}

export interface Section {
  id: string;
  title: string | null;
  locator: Locator;
}

export interface Document {
  id: string;
  format: BookFormat;
  title: string;
  author: string | null;
  sections: Section[];
}

export interface ReaderSession {
  bookId: string;
  document: Document;
  locator: Locator;
}

export interface EngineInfo {
  id: string;
  name: string;
  offline: boolean;
  voiceCloning: boolean;
  streaming: boolean;
}

export interface VoiceProfile {
  id: string;
  name: string;
  engine: string;
  language: string;
  samplePath: string | null;
  createdAt: string;
}

export interface SynthesisRequest {
  text: string;
  voiceId: string;
  rate: number;
  pitch: number;
}

export interface PeerInfo {
  id: string;
  name: string;
  address: string | null;
  port: number;
  lastSeen: string | null;
  trusted: boolean;
}

export interface SyncStatus {
  enabled: boolean;
  running: boolean;
  peerCount: number;
  lastSync: string | null;
}

export interface AccountStatus {
  signedIn: boolean;
  accountId: string | null;
  displayName: string | null;
  deviceCount: number;
}

export interface FeaturesStatus {
  ocr: boolean;
  embeddings: boolean;
  journal: boolean;
  achievements: boolean;
}

export interface KoraErrorPayload {
  code: string;
  message: string;
}
