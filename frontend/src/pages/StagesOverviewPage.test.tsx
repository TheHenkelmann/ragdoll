// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StagesOverviewPage } from "./StagesOverviewPage";
import { authRoutes, metaRoutes, mockRelease, mockStage, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("StagesOverviewPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("renders stages from API", async () => {
    setupMockFetch([
      ...authRoutes(),
      ...metaRoutes(),
      { path: "/releases", response: [mockRelease] },
      { path: "/stages", response: [mockStage] },
    ]);
    renderWithProviders(<StagesOverviewPage />, { route: "/stages" });

    expect(screen.getByText("Stages")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText("dev")).toBeInTheDocument();
    });
  });
});
