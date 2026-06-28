// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModelsPage } from "./ModelsPage";
import { authRoutes, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

const mockStatus = {
  embedding_dim: 1024,
  local: [],
  catalog: [
    {
      name: "BAAI/bge-m3",
      kind: "embed",
      languages: ["multilingual"],
      present: true,
      releases: ["first-release"],
      loaded: false,
      ram_bytes: null,
      custom: false,
    },
    {
      name: "BAAI/bge-reranker-v2-m3",
      kind: "rerank",
      languages: ["multilingual"],
      present: false,
      releases: [],
      loaded: false,
      ram_bytes: null,
      custom: false,
    },
  ],
  required: [],
  missing: ["BAAI/bge-reranker-v2-m3"],
  mismatches: [],
  active_downloads: [],
};

function renderPage() {
  setupMockFetch([
    ...authRoutes(),
    { path: "/models/status", response: mockStatus },
    {
      path: "/models/storage",
      response: {
        model_dir: "/data/models",
        entries: [
          {
            dir_name: "BAAI/bge-m3",
            model_name: "BAAI/bge-m3",
            kind: "canonical",
            size_bytes: 1_048_576,
            complete: true,
            in_use: true,
          },
        ],
      },
    },
  ]);
  return renderWithProviders(<ModelsPage />);
}

describe("ModelsPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("renders catalog rows and missing-model hints", async () => {
    renderPage();

    expect((await screen.findAllByText("BAAI/bge-m3")).length).toBeGreaterThan(0);
    expect(screen.getAllByText("BAAI/bge-reranker-v2-m3").length).toBeGreaterThan(0);
    expect(screen.getByText(/Missing models required by releases/)).toBeInTheDocument();
  });

  it("filters catalog rows by search term", async () => {
    renderPage();
    await screen.findAllByText("BAAI/bge-m3");

    fireEvent.change(screen.getByPlaceholderText("Search models & releases…"), {
      target: { value: "rerank" },
    });

    await waitFor(() => {
      const catalogTable = screen.getAllByRole("table")[0];
      expect(within(catalogTable).queryByText("BAAI/bge-m3")).not.toBeInTheDocument();
      expect(within(catalogTable).getByText("BAAI/bge-reranker-v2-m3")).toBeInTheDocument();
    });
  });

  it("shows model storage entries", async () => {
    renderPage();

    expect(await screen.findByText("Model storage")).toBeInTheDocument();
    expect(await screen.findByText("1.0 MB")).toBeInTheDocument();
  });
});
