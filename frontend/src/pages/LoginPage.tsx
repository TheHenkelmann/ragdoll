// SPDX-License-Identifier: AGPL-3.0-only

import { FormEvent, useEffect, useState } from "react";
import { Navigate, useNavigate, useSearchParams } from "react-router-dom";
import { AuthInfo, publicApi } from "../api/client";
import { useAuth } from "../context/AuthContext";
import { safeRedirect } from "../utils/redirect";

export function LoginPage() {
  const { login, token } = useAuth();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const redirect = safeRedirect(searchParams.get("redirect"));
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void publicApi<AuthInfo>("/auth/info")
      .then((info) => {
        if (info.password_is_default) {
          setEmail(info.default_admin_email);
          setPassword("admin");
        }
      })
      .catch(() => {});
  }, []);

  if (token) return <Navigate to={redirect} replace />;

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    try {
      await login(email, password);
      navigate(redirect, { replace: true });
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center p-6">
      <form onSubmit={onSubmit} className="card w-full max-w-md space-y-4">
        <div className="flex items-center justify-center gap-3">
          <img src="/assets/logo.png" alt="" className="h-10 w-auto" aria-hidden />
          <span className="text-2xl font-semibold">Ragdoll</span>
        </div>
        <p className="text-center text-sm text-[var(--muted)]">Sign in with your email</p>
        <label className="block space-y-1">
          <span className="text-sm">Email</span>
          <input className="input" type="email" value={email} onChange={(e) => setEmail(e.target.value)} required />
        </label>
        <label className="block space-y-1">
          <span className="text-sm">Password</span>
          <input className="input" type="password" value={password} onChange={(e) => setPassword(e.target.value)} required />
        </label>
        {error && <p className="text-sm text-red-400">{error}</p>}
        <button className="btn-primary w-full" type="submit">Sign in</button>
      </form>
    </div>
  );
}
