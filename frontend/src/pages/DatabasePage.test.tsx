// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "react-router-dom";
import { DatabasePage } from "./DatabasePage";
import { authRoutes, metaRoutes, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("DatabasePage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  function renderDatabase() {
    setupMockFetch([
      ...authRoutes(),
      ...metaRoutes(),
      {
        path: "/releases/v1/db/",
        response: (url: string) => {
          if (url.includes("/chunks")) {
            return {
              columns: ["content", "source_id", "id"],
              rows: [{ id: "chk-1", source_id: "src-1", content: "data" }],
              facets: {},
            };
          }
          return {
            columns: ["name", "type", "id"],
            rows: [{ id: "src-1", name: "Doc A", type: "file" }],
            facets: { type: { truncated: false, values: ["file"] } },
          };
        },
      },
    ]);
    return renderWithProviders(
      <Routes>
        <Route path="/releases/:releaseTag/database" element={<DatabasePage />} />
      </Routes>,
      { route: "/releases/v1/database" },
    );
  }

  it("renders table tabs and default sources data", async () => {
    renderDatabase();

    expect(screen.getByText("Database")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "sources" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "webhooks" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "webhook_deliveries" })).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText("Doc A")).toBeInTheDocument();
    });
  });

  it("switches table via tab buttons", async () => {
    renderDatabase();

    await waitFor(() => {
      expect(screen.getByText("Doc A")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "chunks" }));

    await waitFor(() => {
      expect(screen.getByText("data")).toBeInTheDocument();
    });
  });
});
