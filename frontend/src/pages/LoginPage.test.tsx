// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LoginPage } from "./LoginPage";
import { authRoutes, mockAuthInfo, mockAuthStatus, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("LoginPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  beforeEach(() => {
    localStorage.clear();
  });

  it("renders sign-in form", () => {
    setupMockFetch(authRoutes());
    renderWithProviders(<LoginPage />, { route: "/login", token: null });

    expect(screen.getByText("Sign in with your email")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sign in" })).toBeInTheDocument();
  });

  it("prefills default admin credentials when password is default", async () => {
    setupMockFetch([
      { path: "/auth/status", response: mockAuthStatus },
      {
        path: "/auth/info",
        response: { ...mockAuthInfo, password_is_default: true },
      },
      {
        path: "/auth/login",
        method: "POST",
        response: { token: "test-token" },
      },
    ]);
    renderWithProviders(<LoginPage />, { route: "/login", token: null });

    await waitFor(() => {
      expect(screen.getByDisplayValue("admin@ragdoll.ai")).toBeInTheDocument();
      expect(screen.getByDisplayValue("admin")).toBeInTheDocument();
    });
  });

  it("shows error on failed login", async () => {
    setupMockFetch([
      {
        path: "/auth/info",
        response: mockAuthInfo,
      },
      {
        path: "/auth/login",
        method: "POST",
        status: 401,
        response: { error: "invalid credentials" },
      },
    ]);
    renderWithProviders(<LoginPage />, { route: "/login", token: null });

    fireEvent.change(screen.getByLabelText("Email"), { target: { value: "bad@example.com" } });
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "wrong" } });
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

    await waitFor(() => {
      expect(screen.getByText(/401/)).toBeInTheDocument();
    });
  });
});
