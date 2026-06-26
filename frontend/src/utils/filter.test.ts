// SPDX-License-Identifier: AGPL-3.0-only

import { describe, expect, it } from "vitest";
import {
  conditionFromNode,
  conditionsToFilter,
  filterToConditions,
} from "./filter";

describe("filter utils", () => {
  it("builds a single condition filter", () => {
    const filter = conditionsToFilter([{ field: "meta.department", op: "eq", value: "hr" }]);
    expect(filter).toEqual({ field: "meta.department", op: "eq", value: "hr" });
  });

  it("builds an AND filter from multiple conditions", () => {
    const filter = conditionsToFilter([
      { field: "meta.department", op: "eq", value: "hr" },
      { field: "source_id", op: "in", value: '["a","b"]' },
    ]);
    expect(filter).toEqual({
      and: [
        { field: "meta.department", op: "eq", value: "hr" },
        { field: "source_id", op: "in", value: ["a", "b"] },
      ],
    });
  });

  it("roundtrips filter JSON through conditions", () => {
    const raw = { field: "status", op: "eq", value: "completed" };
    const conditions = filterToConditions(raw);
    expect(conditionsToFilter(conditions)).toEqual(raw);
  });

  it("returns undefined for empty conditions", () => {
    expect(conditionsToFilter([])).toBeUndefined();
    expect(conditionsToFilter([{ field: "  ", op: "eq", value: "x" }])).toBeUndefined();
  });

  it("parses in/nin operators from JSON and comma lists", () => {
    expect(
      conditionsToFilter([{ field: "id", op: "in", value: "a, b ,c" }]),
    ).toEqual({ field: "id", op: "in", value: ["a", "b", "c"] });

    expect(
      conditionsToFilter([{ field: "id", op: "nin", value: "not-json" }]),
    ).toEqual({ field: "id", op: "nin", value: ["not-json"] });
  });

  it("coerces boolean and numeric values", () => {
    expect(conditionsToFilter([{ field: "active", op: "eq", value: "true" }])).toEqual({
      field: "active",
      op: "eq",
      value: true,
    });
    expect(conditionsToFilter([{ field: "count", op: "gt", value: "42" }])).toEqual({
      field: "count",
      op: "gt",
      value: 42,
    });
  });

  it("handles exists operator without value", () => {
    expect(conditionsToFilter([{ field: "meta.x", op: "exists", value: "" }])).toEqual({
      field: "meta.x",
      op: "exists",
    });
  });

  it("filterToConditions handles and arrays and invalid input", () => {
    expect(filterToConditions(null)).toEqual([{ field: "", op: "eq", value: "" }]);
    expect(filterToConditions({ and: [{ field: "a", op: "eq", value: 1 }] })).toEqual([
      { field: "a", op: "eq", value: "1" },
    ]);
  });

  it("conditionFromNode serializes arrays and objects", () => {
    expect(conditionFromNode({ field: "tags", op: "in", value: ["a", "b"] })).toEqual({
      field: "tags",
      op: "in",
      value: '["a","b"]',
    });
    expect(conditionFromNode({ field: "meta", op: "eq", value: { k: 1 } })).toEqual({
      field: "meta",
      op: "eq",
      value: '{"k":1}',
    });
    expect(conditionFromNode({ field: "x", op: "exists" })).toEqual({
      field: "x",
      op: "exists",
      value: "",
    });
    expect(conditionFromNode("bad")).toBeNull();
  });
});
