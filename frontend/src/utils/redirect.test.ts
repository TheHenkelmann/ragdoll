// SPDX-License-Identifier: AGPL-3.0-only

import { describe, expect, it } from "vitest";
import { safeRedirect } from "./redirect";

describe("safeRedirect", () => {
  it("defaults invalid redirects to /releases", () => {
    expect(safeRedirect(null)).toBe("/releases");
    expect(safeRedirect("//evil.example")).toBe("/releases");
    expect(safeRedirect("https://evil.example")).toBe("/releases");
  });

  it("keeps valid in-app paths", () => {
    expect(safeRedirect("/releases/first-release/playground")).toBe(
      "/releases/first-release/playground",
    );
  });
});
