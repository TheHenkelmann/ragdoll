// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "react-router-dom";
import { Layout } from "./Layout";
import { DashboardPage } from "../pages/DashboardPage";
import {
  authRoutes,
  metaRoutes,
  mockAnalytics,
  mockRelease,
  mockSystemMetrics,
  setupMockFetch,
} from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("Layout", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  function renderLayout(route = "/releases/v1") {
    setupMockFetch([
      ...authRoutes(),
      ...metaRoutes(),
      { path: "/analytics", response: mockAnalytics },
      { path: "/system-metrics", response: mockSystemMetrics },
    ]);
    return renderWithProviders(
      <Routes>
        <Route path="/releases" element={<Layout />}>
          <Route index element={<div>Releases overview</div>} />
        </Route>
        <Route path="/releases/:releaseTag" element={<Layout />}>
          <Route index element={<DashboardPage />} />
        </Route>
      </Routes>,
      { route },
    );
  }

  it("renders header with navigation breadcrumbs", async () => {
    renderLayout();

    expect(screen.getByText("Ragdoll")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Dashboard" })).toBeInTheDocument();
    });
  });

  it("shows sidebar nav links for release views", async () => {
    renderLayout();

    await waitFor(() => {
      expect(screen.getByRole("link", { name: "Playground" })).toBeInTheDocument();
      expect(screen.getByRole("link", { name: "Sources" })).toBeInTheDocument();
    });
  });

  it("shows unknown release message when tag missing", async () => {
    setupMockFetch([
      ...authRoutes(),
      { path: "/releases", response: [mockRelease] },
      { path: "/stages", response: [] },
    ]);
    renderWithProviders(
      <Routes>
        <Route path="/releases/:releaseTag" element={<Layout />}>
          <Route index element={<DashboardPage />} />
        </Route>
      </Routes>,
      { route: "/releases/unknown-tag" },
    );

    await waitFor(() => {
      expect(screen.getByText("Release not found")).toBeInTheDocument();
    });
  });
});
