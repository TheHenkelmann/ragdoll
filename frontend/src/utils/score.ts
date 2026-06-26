// SPDX-License-Identifier: AGPL-3.0-only

export function parseScore(raw: string): number {
  const n = Number.parseFloat(raw.replace(",", "."));
  if (!Number.isFinite(n)) return 0;
  return Math.min(1, Math.max(0, n));
}
