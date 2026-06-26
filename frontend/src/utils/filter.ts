// SPDX-License-Identifier: AGPL-3.0-only

export type FilterCondition = { field: string; op: string; value: string };

export function conditionsToFilter(conditions: FilterCondition[]): unknown | undefined {
  const valid = conditions.filter((c) => c.field.trim());
  if (valid.length === 0) return undefined;
  const mapped = valid.map((c) => {
    let value: unknown = c.value;
    if (c.op === "in" || c.op === "nin") {
      try {
        value = JSON.parse(c.value || "[]");
      } catch {
        value = c.value.split(",").map((s) => s.trim()).filter(Boolean);
      }
    } else if (c.op === "exists") {
      return { field: c.field, op: c.op };
    } else if (c.value === "true") value = true;
    else if (c.value === "false") value = false;
    else if (c.value !== "" && !Number.isNaN(Number(c.value)) && c.value.trim() !== "") {
      value = Number(c.value);
    }
    return { field: c.field, op: c.op, value };
  });
  if (mapped.length === 1) return mapped[0];
  return { and: mapped };
}

export function filterToConditions(raw: unknown): FilterCondition[] {
  if (!raw || typeof raw !== "object") return [{ field: "", op: "eq", value: "" }];
  const obj = raw as Record<string, unknown>;
  if (Array.isArray(obj.and)) {
    return obj.and.map((item) => conditionFromNode(item)).filter(Boolean) as FilterCondition[];
  }
  const single = conditionFromNode(raw);
  return single ? [single] : [{ field: "", op: "eq", value: "" }];
}

export function conditionFromNode(node: unknown): FilterCondition | null {
  if (!node || typeof node !== "object") return null;
  const n = node as Record<string, unknown>;
  if (typeof n.field !== "string" || typeof n.op !== "string") return null;
  if (n.op === "exists") return { field: n.field, op: n.op, value: "" };
  const value = n.value;
  if (Array.isArray(value)) return { field: n.field, op: n.op, value: JSON.stringify(value) };
  if (typeof value === "object" && value !== null) {
    return { field: n.field, op: n.op, value: JSON.stringify(value) };
  }
  return { field: n.field, op: n.op, value: value == null ? "" : String(value) };
}
