// SPDX-License-Identifier: AGPL-3.0-only

export function defaultStartDate(): string {
  const d = new Date();
  d.setDate(d.getDate() - 13);
  return d.toISOString().slice(0, 10);
}

export function todayDate(): string {
  return new Date().toISOString().slice(0, 10);
}

export function formatLatencyStats(
  stats: { p50: number; p95: number } | undefined,
  hasRequests: boolean,
): { p50: string; p95: string } {
  if (!hasRequests || !stats || (stats.p50 === 0 && stats.p95 === 0)) {
    return { p50: "–", p95: "–" };
  }
  return { p50: String(Math.round(stats.p50)), p95: String(Math.round(stats.p95)) };
}

export function formatBytesGiB(bytes: number): string {
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

export function formatPercent(value: number, digits = 0): string {
  return `${value.toFixed(digits)}%`;
}
