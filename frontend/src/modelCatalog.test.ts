// SPDX-License-Identifier: AGPL-3.0-only

import { describe, expect, it } from "vitest";
import {
  compareCatalogRows,
  downloadSortGroup,
  filterCatalogRows,
  formatRam,
} from "./modelCatalog";

describe("modelCatalog", () => {
  it("formatRam shows dash for null", () => {
    expect(formatRam(null)).toBe("—");
    expect(formatRam(50 * 1024 * 1024)).toMatch(/MB/);
  });

  it("downloadSortGroup orders downloading first", () => {
    expect(downloadSortGroup("a", false, ["a"], "idle")).toBe("downloading");
    expect(downloadSortGroup("b", true, [], "idle")).toBe("present");
    expect(downloadSortGroup("c", false, [], "idle")).toBe("missing");
  });

  it("compareCatalogRows sorts missing after present", () => {
    const active: string[] = [];
    const rowState = {};
    const rows = [
      { name: "z/model", present: false },
      { name: "a/model", present: true },
    ];
    const sorted = [...rows].sort((a, b) => compareCatalogRows(a, b, active, rowState));
    expect(sorted[0].name).toBe("a/model");
    expect(sorted[1].name).toBe("z/model");
  });

  it("filterCatalogRows matches release tags", () => {
    const rows = [
      { name: "BAAI/bge-m3", releases: ["prod", "staging"] },
      { name: "other/model", releases: [] },
    ];
    expect(filterCatalogRows(rows, "prod")).toHaveLength(1);
    expect(filterCatalogRows(rows, "bge")).toHaveLength(1);
  });
});
