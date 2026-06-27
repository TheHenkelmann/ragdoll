// SPDX-License-Identifier: AGPL-3.0-only

type Props = {
  permission: string;
  title?: string;
};

export function PermissionDenied({ permission, title = "Access denied" }: Props) {
  return (
    <div className="card">
      <h2 className="text-xl font-semibold">{title}</h2>
      <p className="mt-2 text-sm text-[var(--muted)]">
        Missing permission: <code className="rounded px-1" style={{ background: "var(--selected)" }}>{permission}</code>
      </p>
    </div>
  );
}
