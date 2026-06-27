// SPDX-License-Identifier: AGPL-3.0-only

const SUPPORTED_FILE_EXTENSIONS = [
  ".txt",
  ".md",
  ".csv",
  ".json",
  ".pdf",
  ".docx",
  ".xlsx",
  ".xlsm",
  ".pptx",
];

function fileExtension(name: string): string | null {
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return null;
  return name.slice(dot).toLowerCase();
}

function hasSupportedExtension(name: string): boolean {
  const ext = fileExtension(name);
  return ext != null && SUPPORTED_FILE_EXTENSIONS.includes(ext);
}

/** Ensures the source name ends with a supported extension (from the selected file). */
export function resolveFileSourceName(customName: string, file: File): string {
  const trimmed = customName.trim();
  if (trimmed && hasSupportedExtension(trimmed)) return trimmed;
  const fileExt = fileExtension(file.name);
  if (!fileExt || !SUPPORTED_FILE_EXTENSIONS.includes(fileExt)) {
    return trimmed || file.name;
  }
  if (!trimmed) return file.name;
  return `${trimmed}${fileExt}`;
}
