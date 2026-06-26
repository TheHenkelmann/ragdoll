// SPDX-License-Identifier: AGPL-3.0-only

import { describe, expect, it, vi, afterEach } from "vitest";
import { defaultStartDate, formatLatencyStats, todayDate } from "./format";

describe("formatLatencyStats", () => {
  it("returns dashes when there are no requests", () => {
    expect(formatLatencyStats({ p50: 10, p95: 20 }, false)).toEqual({ p50: "–", p95: "–" });
  });

  it("rounds latency values when requests exist", () => {
    expect(formatLatencyStats({ p50: 12.4, p95: 98.6 }, true)).toEqual({ p50: "12", p95: "99" });
  });

  it("returns dashes when stats are missing or zero", () => {
    expect(formatLatencyStats(undefined, true)).toEqual({ p50: "–", p95: "–" });
    expect(formatLatencyStats({ p50: 0, p95: 0 }, true)).toEqual({ p50: "–", p95: "–" });
  });
});

describe("date helpers", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("defaultStartDate returns ISO date 13 days ago", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-06-15T12:00:00Z"));
    expect(defaultStartDate()).toBe("2024-06-02");
  });

  it("todayDate returns current ISO date", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-06-15T12:00:00Z"));
    expect(todayDate()).toBe("2024-06-15");
  });
});
