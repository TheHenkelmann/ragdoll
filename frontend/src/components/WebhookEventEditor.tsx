// SPDX-License-Identifier: AGPL-3.0-only

import { WEBHOOK_EVENT_CATALOG } from "../api/client";

type Props = {
  value: string[];
  onChange: (events: string[]) => void;
  idPrefix?: string;
};

export function WebhookEventEditor({ value, onChange, idPrefix = "wh-event" }: Props) {
  const selected = new Set(value);

  function emit(next: Set<string>) {
    onChange([...next]);
  }

  function toggle(event: string) {
    const next = new Set(selected);
    if (next.has(event)) next.delete(event);
    else next.add(event);
    emit(next);
  }

  function toggleSection(events: string[]) {
    const allSelected = events.every((e) => selected.has(e));
    const next = new Set(selected);
    if (allSelected) {
      for (const e of events) next.delete(e);
    } else {
      for (const e of events) next.add(e);
    }
    emit(next);
  }

  return (
    <div
      className="space-y-3 rounded-lg border p-3"
      style={{ borderColor: "var(--border)", background: "var(--surface)" }}
    >
      {WEBHOOK_EVENT_CATALOG.map(({ section, events }) => {
        const ids = events.map((e) => e.id);
        const allSelected = ids.every((id) => selected.has(id));
        const someSelected = ids.some((id) => selected.has(id));
        const sectionId = `${idPrefix}-${section.replace(/\s+/g, "-").toLowerCase()}`;
        return (
          <div key={section} className="space-y-1.5">
            <label className="flex cursor-pointer items-center gap-2 text-sm font-medium">
              <input
                type="checkbox"
                className="rounded"
                checked={allSelected}
                ref={(el) => {
                  if (el) el.indeterminate = someSelected && !allSelected;
                }}
                onChange={() => toggleSection(ids)}
              />
              {section}
            </label>
            <div className="ml-5 space-y-1">
              {events.map((event) => (
                <label
                  key={event.id}
                  className="flex cursor-pointer items-center gap-2 text-sm"
                >
                  <input
                    id={`${sectionId}-${event.id}`}
                    type="checkbox"
                    className="rounded"
                    checked={selected.has(event.id)}
                    onChange={() => toggle(event.id)}
                  />
                  <span>{event.label}</span>
                </label>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}
