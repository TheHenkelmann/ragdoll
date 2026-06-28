// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "react-router-dom";
import { WebhooksPage } from "./WebhooksPage";
import { authRoutes, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

const mockWebhook = {
  id: "wh-1",
  release_id: "rel-1",
  type: "ingest_status",
  url: "https://example.com/hook",
  events: ["completed", "failed"],
  active: true,
  created_at: "2026-01-01T00:00:00Z",
};

function renderPage(extraRoutes: Parameters<typeof setupMockFetch>[0] = []) {
  setupMockFetch([
    ...authRoutes(),
    { path: "/releases/v1/webhooks", response: [mockWebhook] },
    ...extraRoutes,
  ]);
  return renderWithProviders(
    <Routes>
      <Route path="/releases/:releaseTag/webhooks" element={<WebhooksPage />} />
    </Routes>,
    { route: "/releases/v1/webhooks" },
  );
}

describe("WebhooksPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("renders configured webhooks", async () => {
    renderPage();
    expect(await screen.findByText("https://example.com/hook")).toBeInTheDocument();
    expect(screen.getByText(/2 events/)).toBeInTheDocument();
  });

  it("opens create form and requires at least one event", async () => {
    renderPage();

    expect(await screen.findByText("https://example.com/hook")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Create webhook" }));

    expect(screen.getByPlaceholderText("https://example.com/webhook")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create" })).toBeDisabled();
  });

  it("shows test result after send test", async () => {
    vi.spyOn(await import("../api/client"), "testWebhook").mockResolvedValue({
      status_code: 200,
      body: "ok",
    });

    renderPage();
    expect(await screen.findByText("https://example.com/hook")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Send test request" }));

    await waitFor(() => {
      expect(screen.getByText("HTTP 200")).toBeInTheDocument();
    });
  });
});
