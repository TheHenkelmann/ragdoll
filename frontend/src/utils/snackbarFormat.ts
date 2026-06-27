// SPDX-License-Identifier: AGPL-3.0-only

export function formatApiError(err: unknown): { title: string; body: string } {
  const raw = err instanceof Error ? err.message : String(err);
  const match = raw.match(/^(\d{3})\s+([\s\S]*)$/);
  if (match) {
    const status = match[1];
    const body = match[2].trim();
    return { title: `Request failed (${status})`, body };
  }
  if (raw.startsWith("Error: ")) {
    const inner = raw.slice(7);
    const innerMatch = inner.match(/^(\d{3})\s+([\s\S]*)$/);
    if (innerMatch) {
      return { title: `Request failed (${innerMatch[1]})`, body: innerMatch[2].trim() };
    }
    return { title: "Error", body: inner };
  }
  return { title: raw, body: "" };
}

export function pushApiError(
  showError: (title: string, body?: string) => void,
  err: unknown,
) {
  const { title, body } = formatApiError(err);
  showError(title, body || undefined);
}
