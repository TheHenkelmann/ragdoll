// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ReleasesOverviewPage } from "./ReleasesOverviewPage";
import { authRoutes, metaRoutes, mockRelease, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("ReleasesOverviewPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("renders releases from API", async () => {
    setupMockFetch([...authRoutes(), ...metaRoutes(), { path: "/releases", response: [mockRelease] }]);
    renderWithProviders(<ReleasesOverviewPage />, { route: "/releases" });

    expect(screen.getByText("Releases")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText("v1")).toBeInTheDocument();
      expect(screen.getByText(/Stages: dev/)).toBeInTheDocument();
    });
  });
});
