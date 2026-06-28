// SPDX-License-Identifier: AGPL-3.0-only

import { describe, expect, it } from "vitest";
import { applyDownloadEvent } from "./useModelDownloads";

describe("applyDownloadEvent", () => {
  it("maps progress events to percentage and byte counters", () => {
    const next = applyDownloadEvent({ status: "idle" }, {
      event: "progress",
      name: "BAAI/bge-m3",
      bytes: 50,
      total: 100,
      message: "Downloading…",
    });
    expect(next.status).toBe("downloading");
    expect(next.progress).toBe(50);
    expect(next.progressBytes).toBe(50);
    expect(next.progressTotal).toBe(100);
  });

  it("caps progress below 100 until complete", () => {
    const next = applyDownloadEvent({ status: "downloading" }, {
      event: "progress",
      name: "BAAI/bge-m3",
      bytes: 999,
      total: 1000,
      message: "Almost done",
    });
    expect(next.progress).toBe(99);
  });

  it("marks complete with latency message", () => {
    const next = applyDownloadEvent({ status: "testing" }, {
      event: "complete",
      name: "BAAI/bge-m3",
      latency_ms: 42,
    });
    expect(next.status).toBe("ready");
    expect(next.message).toContain("42");
  });

  it("preserves cancellable flag from cancellable events", () => {
    const next = applyDownloadEvent(
      { status: "downloading", cancellable: false },
      { event: "cancellable", name: "BAAI/bge-m3", cancellable: true },
    );
    expect(next.cancellable).toBe(true);
  });

  it("maps error and cancelled terminal states", () => {
    expect(
      applyDownloadEvent(
        { status: "downloading" },
        { event: "error", name: "BAAI/bge-m3", message: "boom" },
      ).status,
    ).toBe("error");
    expect(
      applyDownloadEvent(
        { status: "downloading" },
        { event: "cancelled", name: "BAAI/bge-m3" },
      ).status,
    ).toBe("cancelled");
  });
});
