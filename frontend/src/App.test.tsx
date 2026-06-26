// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import {
  authRoutes,
  metaRoutes,
  mockAnalytics,
  mockRelease,
  setupMockFetch,
} from "./test/mockApi";
import { renderWithProviders } from "./test/renderWithProviders";

describe("App routing", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("redirects unauthenticated users to login", async () => {
    setupMockFetch(authRoutes());
    renderWithProviders(<App />, { route: "/releases", token: null });

    await waitFor(() => {
      expect(screen.getByText("Sign in with your email")).toBeInTheDocument();
    });
  });

  it("renders releases overview when authenticated", async () => {
    setupMockFetch([...authRoutes(), ...metaRoutes(), { path: "/releases", response: [mockRelease] }]);
    renderWithProviders(<App />, { route: "/releases" });

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Releases" })).toBeInTheDocument();
      expect(screen.getByText("v1")).toBeInTheDocument();
    });
  });

  it("renders dashboard for release route", async () => {
    setupMockFetch([
      ...authRoutes(),
      ...metaRoutes(),
      { path: "/analytics", response: mockAnalytics },
    ]);
    renderWithProviders(<App />, { route: "/releases/v1" });

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Dashboard" })).toBeInTheDocument();
    });
  });

  it("renders not found page for unknown protected routes", async () => {
    setupMockFetch(authRoutes());
    renderWithProviders(<App />, { route: "/does-not-exist" });

    await waitFor(() => {
      expect(screen.getByText("Page not found")).toBeInTheDocument();
    });
  });
});
