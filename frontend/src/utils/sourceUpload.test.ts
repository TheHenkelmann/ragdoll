// SPDX-License-Identifier: AGPL-3.0-only

import { describe, expect, it } from "vitest";
import { resolveFileSourceName } from "./sourceUpload";

describe("resolveFileSourceName", () => {
  it("uses filename when custom name is empty", () => {
    const file = new File(["x"], "report.pdf", { type: "application/pdf" });
    expect(resolveFileSourceName("", file)).toBe("report.pdf");
  });

  it("appends extension when custom name lacks one", () => {
    const file = new File(["x"], "report.pdf", { type: "application/pdf" });
    expect(resolveFileSourceName("mydoc", file)).toBe("mydoc.pdf");
  });

  it("keeps custom name when it already has a supported extension", () => {
    const file = new File(["x"], "report.pdf", { type: "application/pdf" });
    expect(resolveFileSourceName("custom.pdf", file)).toBe("custom.pdf");
  });
});
