// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ObjectOverviewPage } from "./ObjectOverviewPage";
import { authRoutes, metaRoutes, mockRelease, mockStage, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("ObjectOverviewPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("shows empty state when no releases match search", async () => {
    setupMockFetch([...authRoutes(), ...metaRoutes(), { path: "/releases", response: [mockRelease] }]);
    renderWithProviders(<ObjectOverviewPage kind="release" />, { route: "/releases" });

    await waitFor(() => {
      expect(screen.getByText("v1")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText("Search releases"), {
      target: { value: "zzz-not-found" },
    });

    expect(screen.getByText("No releases found.")).toBeInTheDocument();
  });

  it("loads stages overview with release selector", async () => {
    setupMockFetch([
      ...authRoutes(),
      { path: "/releases", response: [mockRelease] },
      { path: "/stages", response: [mockStage] },
    ]);
    renderWithProviders(<ObjectOverviewPage kind="stage" />, { route: "/stages" });

    await waitFor(() => {
      expect(screen.getByText("dev")).toBeInTheDocument();
      expect(screen.getByRole("combobox")).toHaveValue("v1");
    });
  });
});
