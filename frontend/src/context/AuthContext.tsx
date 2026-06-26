// SPDX-License-Identifier: AGPL-3.0-only

import { createContext, useContext, useEffect, useMemo, useState } from "react";
import { api, AuthStatus, getToken, publicApi, setToken } from "../api/client";

type AuthContextValue = {
  token: string | null;
  status: AuthStatus | null;
  login: (email: string, password: string) => Promise<void>;
  logout: () => void;
  refresh: () => Promise<void>;
};

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [token, setTokenState] = useState<string | null>(getToken());
  const [status, setStatus] = useState<AuthStatus | null>(null);

  const refresh = async () => {
    if (!getToken()) {
      setStatus(null);
      return;
    }
    const next = await api<AuthStatus>("/auth/status");
    setStatus(next);
  };

  useEffect(() => {
    if (token) {
      void refresh().catch((err) => {
        if (err instanceof Error && err.message === "unauthorized") {
          setToken(null);
          setTokenState(null);
          setStatus(null);
        }
      });
    }
  }, [token]);

  const value = useMemo(
    () => ({
      token,
      status,
      async login(email: string, password: string) {
        const res = await publicApi<{ token: string }>("/auth/login", {
          method: "POST",
          body: JSON.stringify({ email, password }),
        });
        setToken(res.token);
        setTokenState(res.token);
      },
      logout() {
        setToken(null);
        setTokenState(null);
        setStatus(null);
      },
      refresh,
    }),
    [token, status],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("AuthProvider missing");
  return ctx;
}
