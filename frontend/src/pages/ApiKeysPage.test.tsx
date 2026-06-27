// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiKeysPage } from "./ApiKeysPage";
import { authRoutes, metaRoutes, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("ApiKeysPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("blocks duplicate API key names in the UI", async () => {
    setupMockFetch([
      ...authRoutes(),
      ...metaRoutes(),
      {
        path: "/api_keys",
        response: [
          {
            id: "k1",
            name: "existing",
            permissions: [],
            created_at: "2024-01-01T00:00:00Z",
          },
        ],
      },
    ]);
    renderWithProviders(<ApiKeysPage />);

    await waitFor(() => {
      expect(screen.getByText("existing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Create API key" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "existing" },
    });
    fireEvent.submit(screen.getByRole("button", { name: "Create" }).closest("form")!);

    await waitFor(() => {
      expect(screen.getByText('An API key named "existing" already exists')).toBeInTheDocument();
    });
  });
});
