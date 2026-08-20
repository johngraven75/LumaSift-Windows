export type SelectionType = "video" | "audio" | "document" | "image";

export interface ScanRequest {
  sources: string[];
  selectedTypes: SelectionType[];
}

export interface NasConnectRequest {
  uncPath: string;
  username: string;
  password: string;
  rememberConnection: boolean;
}

export interface ScanProgress {
  scanning: boolean;
  phase: string;
  current: number;
  total: number;
  percentage: number;
  currentPath?: string;
  filesConsidered: number;
  message: string;
  error?: string;
}

export interface QualityEvidence {
  width?: number;
  height?: number;
  pixelCount: number;
  bitrate?: number;
  bitDepth?: number;
  durationMillis?: number;
  fileSizeBytes: number;
  reasons: string[];
}

export interface Candidate {
  id: string;
  filePath: string;
  displayName: string;
  mediaKind: string;
  exactHash: string;
  qualityScore: number;
  quality: QualityEvidence;
  disposition: string;
  dispositionDetail: string;
  quarantinePath?: string;
}

export interface DuplicateGroup {
  id: string;
  exactHash: string;
  winnerId: string;
  reclaimableBytes: number;
  candidates: Candidate[];
}

export interface Disposition {
  occurredAt: string;
  filePath: string;
  displayName: string;
  disposition: string;
  detail: string;
}

export interface ResolutionPlan {
  id: string;
  status: string;
  selectedTypes: SelectionType[];
  createdAt: string;
  groups: DuplicateGroup[];
  reclaimableBytes: number;
  queuedFileCount: number;
  dispositions: Disposition[];
}

export const ALL_SELECTION_TYPES: SelectionType[] = ["video", "audio", "document", "image"];

export const selectionLabel: Record<SelectionType, string> = {
  video: "Videos",
  audio: "MP3 audio",
  document: "DOCX & PDF",
  image: "Images"
};
