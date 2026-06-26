// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "react-router-dom";
import { SourcesPage } from "./SourcesPage";
import {
  authRoutes,
  metaRoutes,
  mockChunk,
  mockSource,
  setupMockFetch,
} from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("SourcesPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  function renderSources() {
    setupMockFetch([
      ...authRoutes(),
      ...metaRoutes(),
      { path: "/releases/v1/sources", response: [mockSource] },
      { path: "/releases/v1/chunks", response: [mockChunk] },
    ]);
    return renderWithProviders(
      <Routes>
        <Route path="/releases/:releaseTag/sources" element={<SourcesPage />} />
      </Routes>,
      { route: "/releases/v1/sources" },
    );
  }

  it("lists sources from API", async () => {
    renderSources();

    await waitFor(() => {
      expect(screen.getByText("Doc A")).toBeInTheDocument();
    });
  });

  it("filters sources by search query", async () => {
    renderSources();

    await waitFor(() => {
      expect(screen.getByText("Doc A")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText("Filter sources by name or id"), {
      target: { value: "nonexistent" },
    });

    expect(screen.queryByText("Doc A")).not.toBeInTheDocument();
  });

  it("loads chunks when source is selected", async () => {
    renderSources();

    await waitFor(() => {
      expect(screen.getByText("Doc A")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("Doc A"));

    await waitFor(() => {
      expect(screen.getByText("Sample chunk content")).toBeInTheDocument();
    });
  });
});
