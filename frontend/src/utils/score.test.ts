// SPDX-License-Identifier: AGPL-3.0-only

import { describe, expect, it } from "vitest";
import { parseScore } from "./score";

describe("parseScore", () => {
  it("parses comma decimals and clamps to 0..1", () => {
    expect(parseScore("0,85")).toBe(0.85);
    expect(parseScore("1.5")).toBe(1);
    expect(parseScore("-0.2")).toBe(0);
    expect(parseScore("not-a-number")).toBe(0);
  });
});
