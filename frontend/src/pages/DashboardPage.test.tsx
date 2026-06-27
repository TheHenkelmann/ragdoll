// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "react-router-dom";
import { DashboardPage } from "./DashboardPage";
import {
  authRoutes,
  metaRoutes,
  mockAnalytics,
  mockSystemMetrics,
  setupMockFetch,
} from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("DashboardPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  function renderDashboard(route = "/releases/v1") {
    setupMockFetch([
      ...authRoutes(),
      ...metaRoutes(),
      { path: "/analytics", response: mockAnalytics },
      { path: "/system-metrics", response: mockSystemMetrics },
    ]);
    return renderWithProviders(
      <Routes>
        <Route path="/releases/:releaseTag" element={<DashboardPage />} />
        <Route path="/stages/:stageTag" element={<DashboardPage />} />
      </Routes>,
      { route },
    );
  }

  it("renders dashboard KPIs from analytics API", async () => {
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Dashboard" })).toBeInTheDocument();
      expect(screen.getByText("42")).toBeInTheDocument();
    });
  });

  it("renders stage lens snapshot hint", async () => {
    renderDashboard("/stages/dev");

    await waitFor(() => {
      expect(screen.getByText("Snapshot of current linked release")).toBeInTheDocument();
    });
  });

  it("shows metadata and query chunk sections when data present", async () => {
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("Query result chunks")).toBeInTheDocument();
      expect(screen.getByText("department")).toBeInTheDocument();
      expect(screen.getByText("topic")).toBeInTheDocument();
    });
  });
});
