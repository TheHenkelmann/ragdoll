// SPDX-License-Identifier: AGPL-3.0-only

import { FormEvent, useState } from "react";
import { api } from "../api/client";
import { PasswordField } from "../components/PasswordField";
import { useAuth } from "../context/AuthContext";
import { useSnackbar } from "../context/SnackbarContext";
import { useNavigate } from "react-router-dom";
import { evaluatePassword } from "../utils/password";
import { formatApiError } from "../utils/snackbarFormat";

export function ProfilePage() {
  const { status, refresh, logout } = useAuth();
  const snackbar = useSnackbar();
  const navigate = useNavigate();
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [busy, setBusy] = useState(false);

  async function changePassword(e: FormEvent) {
    e.preventDefault();
    if (newPassword !== confirmPassword) {
      snackbar.error("New passwords do not match");
      return;
    }
    if (!evaluatePassword(newPassword).isValid) {
      snackbar.error("Password does not meet the strength requirements");
      return;
    }
    setBusy(true);
    try {
      await api("/auth/password", {
        method: "PATCH",
        body: JSON.stringify({
          current_password: currentPassword,
          new_password: newPassword,
        }),
      });
      setCurrentPassword("");
      setNewPassword("");
      setConfirmPassword("");
      snackbar.success("Password updated successfully.");
      await refresh();
    } catch (err) {
      const { title, body } = formatApiError(err);
      snackbar.error(title, body || undefined);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto max-w-lg space-y-6">
      <h2 className="text-2xl font-semibold">Profile</h2>

      <div className="card space-y-2">
        <div className="text-sm text-[var(--muted)]">Email</div>
        <div className="font-medium">{status?.email ?? "—"}</div>
        <div className="text-sm text-subtle">
          {status?.is_superadmin ? "Superadmin" : "User"}
          {status?.password_is_default && " · default password active"}
        </div>
      </div>

      <div className="card space-y-4">
        <h3 className="text-lg font-medium">Change password</h3>
        {status?.is_superadmin ? (
          <p className="text-sm text-[var(--muted)]">
            Not available in the UI for superadmins. Set the environment variable{" "}
            <code className="rounded px-1" style={{ background: "var(--selected)" }}>
              RAGDOLL_SUPERADMIN_PW
            </code>
            .
          </p>
        ) : (
          <form className="space-y-4" onSubmit={(e) => void changePassword(e)}>
            <PasswordField
              label="Current password"
              value={currentPassword}
              onChange={setCurrentPassword}
              required
              autoComplete="current-password"
            />
            <PasswordField
              label="New password"
              value={newPassword}
              onChange={setNewPassword}
              required
              autoComplete="new-password"
              showStrength
              showGenerate
            />
            <PasswordField
              label="Confirm new password"
              value={confirmPassword}
              onChange={setConfirmPassword}
              required
              autoComplete="new-password"
            />
            <button type="submit" className="btn-primary" disabled={busy}>
              Update password
            </button>
          </form>
        )}
      </div>

      <button
        type="button"
        className="btn-secondary"
        onClick={() => {
          logout();
          navigate("/login");
        }}
      >
        Log out
      </button>
    </div>
  );
}
