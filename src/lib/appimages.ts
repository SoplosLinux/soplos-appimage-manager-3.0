// Shared types for the AppImage domain, mirroring the serde field names in
// src-tauri/src/commands/appimages.rs and updates.rs.

export interface AppImage {
  id: string;
  name: string;
  filePath: string;
  iconPath: string | null;
  iconPreview: string | null;
  desktopFilePath: string | null;
  appimageType: string;
  size: number;
  version: string;
  comment: string;
  categories: string;
  updateUrl: string;
  runAsRoot: boolean;
}

export interface AppImageInfoInput {
  name: string;
  version: string;
  comment: string;
  categories: string;
  updateUrl: string;
  runAsRoot: boolean;
}

export type UpdateMode = "api" | "direct" | "tool" | "none";

export interface UpdateCheckResult {
  hasUpdate: boolean | null;
  message: string;
  latestVersion: string;
  downloadUrl: string;
  assetName: string;
  assetSize: number;
}
