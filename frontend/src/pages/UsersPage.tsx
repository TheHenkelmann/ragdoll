// SPDX-License-Identifier: AGPL-3.0-only

import { FormEvent, useEffect, useState } from "react";
import { FORCED_PERMISSION, UserRecord, api, optionalPermissionCount } from "../api/client";
import { PasswordField } from "../components/PasswordField";
import { PermissionDenied } from "../components/PermissionDenied";
import { PermissionEditor } from "../components/PermissionEditor";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";
import { useSnackbar } from "../context/SnackbarContext";
import { evaluatePassword } from "../utils/password";
import { formatApiError } from "../utils/snackbarFormat";

export function UsersPage() {
  const snackbar = useSnackbar();
  const { can, ready } = usePermissions();
  const canRead = can(PERM.users.read);
  const canWrite = can(PERM.users.write);
  const canDelete = can(PERM.users.delete);
  const [users, setUsers] = useState<UserRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [createPermissions, setCreatePermissions] = useState<string[]>([FORCED_PERMISSION]);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [editUser, setEditUser] = useState<UserRecord | null>(null);
  const [editEmail, setEditEmail] = useState("");
  const [editPassword, setEditPassword] = useState("");
  const [editPermissions, setEditPermissions] = useState<string[]>([]);

  const reload = () => {
    if (!canRead) {
      setLoading(false);
      return;
    }
    setLoading(true);
    void api<UserRecord[]>("/users")
      .then(setUsers)
      .catch((err) => {
        const { title, body } = formatApiError(err);
        snackbar.error(title, body || undefined);
      })
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    if (!ready) return;
    reload();
  }, [ready, canRead]);

  function openCreate() {
    setCreating(true);
    setEmail("");
    setPassword("");
    setCreatePermissions([FORCED_PERMISSION]);
  }

  async function createUser(e: FormEvent) {
    e.preventDefault();
    if (!evaluatePassword(password).isValid) {
      snackbar.error("Password does not meet the strength requirements");
      return;
    }
    if (optionalPermissionCount(createPermissions) === 0) {
      snackbar.error("Select at least one permission besides releases:read");
      return;
    }
    try {
      await api("/users", {
        method: "POST",
        body: JSON.stringify({ email, password, permissions: createPermissions }),
      });
      setEmail("");
      setPassword("");
      setCreatePermissions([FORCED_PERMISSION]);
      setCreating(false);
      reload();
    } catch (err) {
      const { title, body } = formatApiError(err);
      snackbar.error(title, body || undefined);
    }
  }

  function openEdit(user: UserRecord) {
    setEditUser(user);
    setEditEmail(user.email);
    setEditPassword("");
    setEditPermissions(user.permissions ?? []);
  }

  async function saveEdit(e: FormEvent) {
    e.preventDefault();
    if (!editUser) return;
    const body: { email?: string; password?: string; permissions?: string[] } = {};
    if (editEmail !== editUser.email) body.email = editEmail;
    if (editPassword) {
      if (!evaluatePassword(editPassword).isValid) {
        snackbar.error("Password does not meet the strength requirements");
        return;
      }
      body.password = editPassword;
    }
    if (optionalPermissionCount(editPermissions) === 0) {
      snackbar.error("Select at least one permission besides releases:read");
      return;
    }
    const permsChanged =
      JSON.stringify([...editPermissions].sort()) !==
      JSON.stringify([...(editUser.permissions ?? [])].sort());
    if (permsChanged) body.permissions = editPermissions;
    if (!body.email && !body.password && !body.permissions) {
      setEditUser(null);
      return;
    }
    try {
      await api(`/users/${editUser.id}`, {
        method: "PATCH",
        body: JSON.stringify(body),
      });
      setEditUser(null);
      reload();
    } catch (err) {
      const { title, body } = formatApiError(err);
      snackbar.error(title, body || undefined);
    }
  }

  if (ready && !canRead) {
    return <PermissionDenied permission={PERM.users.read} />;
  }

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">Users</h2>

      {!creating ? (
        <button type="button" className="btn-primary" disabled={!canWrite} onClick={openCreate}>
          Create user
        </button>
      ) : (
        <form className="card max-w-lg space-y-3" onSubmit={(e) => void createUser(e)}>
          <label className="block space-y-1 text-sm">
            <span>Email</span>
            <input
              className="input"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
            />
          </label>
          <PasswordField
            label="Password"
            value={password}
            onChange={setPassword}
            required
            autoComplete="new-password"
            showStrength
            showGenerate
          />
          <PermissionEditor
            idPrefix="create-user"
            value={createPermissions}
            onChange={setCreatePermissions}
          />
          <div className="flex items-center gap-2">
            <button
              type="submit"
              className="btn-primary"
              disabled={optionalPermissionCount(createPermissions) === 0}
            >
              Create
            </button>
            <button type="button" className="btn-secondary" onClick={() => setCreating(false)}>
              Cancel
            </button>
            {optionalPermissionCount(createPermissions) === 0 && (
              <span className="text-xs text-[var(--muted)]">
                Select at least one permission besides releases:read
              </span>
            )}
          </div>
        </form>
      )}

      <div className="space-y-2">
        {loading && <p className="text-sm text-[var(--muted)]">Loading…</p>}
        {!loading && users.length === 0 && (
          <p className="text-sm text-[var(--muted)]">No users found.</p>
        )}
        {users.map((u) => (
          <div
            key={u.id}
            className="flex items-center gap-3 rounded-lg border px-4 py-3"
            style={{ borderColor: "var(--border)", background: "var(--surface)" }}
          >
            <div className="min-w-0 flex-1">
              <div className="font-medium">{u.email}</div>
              <div className="text-xs text-subtle">
                {u.is_superadmin ? "Superadmin" : "User"}
                {!u.is_superadmin && (u.permissions?.length ?? 0) > 0 && (
                  <> · {u.permissions.length} permissions</>
                )}
                {" · "}
                {u.created_at}
              </div>
            </div>
            {!u.is_superadmin && (
              <button
                type="button"
                className="btn-secondary shrink-0"
                disabled={!canWrite}
                onClick={() => openEdit(u)}
              >
                Edit
              </button>
            )}
            <button
              type="button"
              className="btn-danger shrink-0"
              disabled={u.is_superadmin || !canDelete}
              title={u.is_superadmin ? "Cannot delete superadmin" : undefined}
              onClick={() => setDeleteId(u.id)}
            >
              Delete
            </button>
          </div>
        ))}
      </div>

      {editUser && (
        <div className="modal-overlay" onClick={() => setEditUser(null)}>
          <form
            className="card w-full max-w-lg space-y-4"
            onClick={(e) => e.stopPropagation()}
            onSubmit={(e) => void saveEdit(e)}
          >
            <h3 className="text-lg font-semibold">Edit user</h3>
            <label className="block space-y-1 text-sm">
              <span>Email</span>
              <input
                className="input"
                type="email"
                value={editEmail}
                onChange={(e) => setEditEmail(e.target.value)}
                required
              />
            </label>
            <PasswordField
              label="New password (optional)"
              value={editPassword}
              onChange={setEditPassword}
              autoComplete="new-password"
              showStrength
              showGenerate
            />
            <PermissionEditor
              idPrefix={`edit-user-${editUser.id}`}
              value={editPermissions}
              onChange={setEditPermissions}
            />
            <div className="flex items-center justify-end gap-2">
              {optionalPermissionCount(editPermissions) === 0 && (
                <span className="mr-auto text-xs text-[var(--muted)]">
                  Select at least one permission besides releases:read
                </span>
              )}
              <button type="button" className="btn-secondary" onClick={() => setEditUser(null)}>
                Cancel
              </button>
              <button
                type="submit"
                className="btn-primary"
                disabled={optionalPermissionCount(editPermissions) === 0}
              >
                Save
              </button>
            </div>
          </form>
        </div>
      )}

      {deleteId && (
        <div className="modal-overlay" onClick={() => setDeleteId(null)}>
          <div className="card w-full max-w-md space-y-4" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-semibold">Delete user?</h3>
            <p className="text-sm text-[var(--muted)]">This cannot be undone.</p>
            <div className="flex justify-end gap-2">
              <button type="button" className="btn-secondary" onClick={() => setDeleteId(null)}>
                Cancel
              </button>
              <button
                type="button"
                className="btn-danger"
                onClick={() => {
                  void api(`/users/${deleteId}`, { method: "DELETE" })
                    .then(() => {
                      setDeleteId(null);
                      reload();
                    })
                    .catch((err) => {
                      const { title, body } = formatApiError(err);
                      snackbar.error(title, body || undefined);
                    });
                }}
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
