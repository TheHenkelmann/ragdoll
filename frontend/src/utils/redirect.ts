// SPDX-License-Identifier: AGPL-3.0-only

export function safeRedirect(raw: string | null): string {
  if (!raw || !raw.startsWith("/") || raw.startsWith("//")) return "/releases";
  return raw;
}
