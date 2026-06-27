// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "react-router-dom";
import { SettingsPage } from "./SettingsPage";
import { authRoutes, metaRoutes, mockSettings, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

const mockModelsStatus = {
  embedding_dim: 1024,
  local: [],
  catalog: [
    {
      name: "embed-model",
      kind: "embed",
      languages: ["en"],
      present: true,
      releases: [],
      loaded: false,
      ram_bytes: null,
      custom: false,
    },
    {
      name: "updated-model",
      kind: "embed",
      languages: ["en"],
      present: true,
      releases: [],
      loaded: false,
      ram_bytes: null,
      custom: false,
    },
    {
      name: "rerank-model",
      kind: "rerank",
      languages: ["en"],
      present: true,
      releases: [],
      loaded: false,
      ram_bytes: null,
      custom: false,
    },
  ],
  required: [],
  missing: [],
  mismatches: [],
  active_downloads: [],
};

const mockJobsIdle = {
  summary: { total: 0, pending: 0, processing: 0, completed: 0, failed: 0, active: 0 },
};

function settingsRoutes() {
  return [
    { path: "/releases/v1/settings", response: mockSettings },
    {
      path: "/releases/v1/settings",
      method: "PATCH",
      response: { ...mockSettings, embedding_model: "updated-model" },
    },
    { path: "/models/status", response: mockModelsStatus },
    { path: "/releases/v1/ingest_jobs", response: mockJobsIdle },
    {
      path: "/releases/v1/chunks",
      response: [{ id: "c1", source_id: "s1", content: "x", metadata: {} }],
    },
    {
      path: "/releases/v1/reindex",
      method: "POST",
      response: {
        batch_id: "batch-1",
        items: [
          {
            index: 0,
            status: 200,
            result: { source_id: "s1", job_id: "j1" },
          },
        ],
      },
    },
  ];
}

describe("SettingsPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  function renderSettings() {
    setupMockFetch([...authRoutes(), ...metaRoutes(), ...settingsRoutes()]);
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

  it("links to the global models page for downloads", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByRole("link", { name: "Models page" })).toHaveAttribute("href", "/models");
    });
  });

  it("allows save when selected models are downloaded", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByDisplayValue("embed-model")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByDisplayValue("embed-model"), {
      target: { value: "updated-model" },
    });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Save" })).not.toBeDisabled();
    });
  });
});
