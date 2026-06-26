// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useState } from "react";
import {
  filterToConditions,
  conditionsToFilter,
  type FilterCondition,
} from "../utils/filter";

type Condition = FilterCondition;

const OPS = ["eq", "ne", "gt", "gte", "lt", "lte", "in", "nin", "contains", "exists"];

type Props = {
  value: string;
  onChange: (json: string) => void;
};

export function FilterBuilder({ value, onChange }: Props) {
  const [conditions, setConditions] = useState<Condition[]>([{ field: "", op: "eq", value: "" }]);
  const [jsonText, setJsonText] = useState(value);
  const [jsonValid, setJsonValid] = useState(true);

  useEffect(() => {
    if (!value.trim()) {
      setConditions([{ field: "", op: "eq", value: "" }]);
      setJsonText("");
      setJsonValid(true);
      return;
    }
    try {
      const parsed = JSON.parse(value) as unknown;
      const pretty = JSON.stringify(parsed, null, 2);
      setJsonText(pretty);
      setJsonValid(true);
      setConditions(filterToConditions(parsed));
    } catch {
      setJsonText(value);
      setJsonValid(false);
    }
  }, [value]);

  function pushBuilder(next: Condition[]) {
    setConditions(next);
    const filter = conditionsToFilter(next);
    onChange(filter ? JSON.stringify(filter) : "");
  }

  function onJsonEdit(text: string) {
    setJsonText(text);
    if (!text.trim()) {
      setJsonValid(true);
      setConditions([{ field: "", op: "eq", value: "" }]);
      onChange("");
      return;
    }
    try {
      const parsed = JSON.parse(text) as unknown;
      setJsonValid(true);
      setConditions(filterToConditions(parsed));
      onChange(JSON.stringify(parsed));
    } catch {
      setJsonValid(false);
    }
  }

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <div className="space-y-3">
        <div className="text-sm font-medium">Filter builder</div>
        {conditions.map((c, idx) => (
          <div key={idx} className="grid gap-2 rounded-lg border p-3" style={{ borderColor: "var(--border)" }}>
            <input className="input text-sm" placeholder="field (e.g. meta.department)" value={c.field} onChange={(e) => {
              const next = [...conditions];
              next[idx] = { ...c, field: e.target.value };
              pushBuilder(next);
            }} />
            <div className="grid grid-cols-2 gap-2">
              <select className="input text-sm" value={c.op} onChange={(e) => {
                const next = [...conditions];
                next[idx] = { ...c, op: e.target.value };
                pushBuilder(next);
              }}>
                {OPS.map((op) => <option key={op} value={op}>{op}</option>)}
              </select>
              <input className="input text-sm" placeholder="value" value={c.value} disabled={c.op === "exists"} onChange={(e) => {
                const next = [...conditions];
                next[idx] = { ...c, value: e.target.value };
                pushBuilder(next);
              }} />
            </div>
            {conditions.length > 1 && (
              <button type="button" className="text-xs text-[var(--muted)] hover:text-[var(--text)]" onClick={() => pushBuilder(conditions.filter((_, i) => i !== idx))}>
                Remove
              </button>
            )}
          </div>
        ))}
        <button type="button" className="btn-secondary text-sm" onClick={() => pushBuilder([...conditions, { field: "", op: "eq", value: "" }])}>
          Add condition (AND)
        </button>
      </div>
      <div className="space-y-2">
        <div className="text-sm font-medium">JSON {!jsonValid && <span className="text-red-400">(invalid — fix to sync)</span>}</div>
        <textarea
          className="input min-h-48 font-mono text-xs"
          value={jsonText}
          onChange={(e) => onJsonEdit(e.target.value)}
          placeholder='{"field":"meta.department","op":"eq","value":"hr"}'
        />
      </div>
    </div>
  );
}
