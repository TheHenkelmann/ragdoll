// SPDX-License-Identifier: AGPL-3.0-only

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getToken, setToken } from "../api/client";
import { AuthProvider, useAuth } from "./AuthContext";

function AuthProbe() {
  const { token, status, login, logout } = useAuth();
  return (
    <div>
      <span data-testid="token">{token ?? "none"}</span>
      <span data-testid="email">{status?.email ?? "none"}</span>
      <button type="button" onClick={() => void login("admin@ragdoll.ai", "admin")}>
        login
      </button>
      <button type="button" onClick={logout}>
        logout
      </button>
    </div>
  );
}

describe("AuthContext", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("loads stored token on mount", () => {
    setToken("stored-token");
    render(
      <AuthProvider>
        <AuthProbe />
      </AuthProvider>,
    );
    expect(screen.getByTestId("token")).toHaveTextContent("stored-token");
  });

  it("login stores token from API response", async () => {
    vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
      const url = String(input);
      if (url.endsWith("/auth/login") && init?.method === "POST") {
        return new Response(JSON.stringify({ token: "fresh-token" }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
      if (url.endsWith("/auth/status")) {
        return new Response(
          JSON.stringify({
            email: "admin@ragdoll.ai",
            is_superadmin: true,
            password_is_default: true,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        );
      }
      throw new Error(`unexpected fetch: ${url}`);
    });

    render(
      <AuthProvider>
        <AuthProbe />
      </AuthProvider>,
    );

    await act(async () => {
      screen.getByText("login").click();
    });

    await waitFor(() => {
      expect(getToken()).toBe("fresh-token");
      expect(screen.getByTestId("email")).toHaveTextContent("admin@ragdoll.ai");
    });
  });

  it("logout clears token and status", async () => {
    setToken("to-clear");
    render(
      <AuthProvider>
        <AuthProbe />
      </AuthProvider>,
    );

    await act(async () => {
      screen.getByText("logout").click();
    });

    expect(getToken()).toBeNull();
    expect(screen.getByTestId("token")).toHaveTextContent("none");
    expect(screen.getByTestId("email")).toHaveTextContent("none");
  });

  it("throws when useAuth is used outside provider", () => {
    expect(() => render(<AuthProbe />)).toThrow("AuthProvider missing");
  });
});
