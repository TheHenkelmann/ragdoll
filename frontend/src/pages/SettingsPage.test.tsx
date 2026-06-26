// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "react-router-dom";
import { SettingsPage } from "./SettingsPage";
import { authRoutes, metaRoutes, mockSettings, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("SettingsPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  function renderSettings() {
    setupMockFetch([
      ...authRoutes(),
      ...metaRoutes(),
      { path: "/releases/v1/settings", response: mockSettings },
      {
        path: "/releases/v1/settings",
        method: "PATCH",
        response: { ...mockSettings, embedding_model: "updated-model" },
      },
    ]);
    return renderWithProviders(
      <Routes>
        <Route path="/releases/:releaseTag/settings" element={<SettingsPage />} />
      </Routes>,
      { route: "/releases/v1/settings" },
    );
  }

  it("loads and displays settings", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByDisplayValue("embed-model")).toBeInTheDocument();
      expect(screen.getByDisplayValue("rerank-model")).toBeInTheDocument();
    });
  });

  it("saves settings via PATCH", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByDisplayValue("embed-model")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByDisplayValue("embed-model"), {
      target: { value: "updated-model" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(screen.getByText("Saved")).toBeInTheDocument();
    });
  });
});
