// SPDX-License-Identifier: AGPL-3.0-only

import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "react-router-dom";
import { PlaygroundPage } from "./PlaygroundPage";
import {
  authRoutes,
  metaRoutes,
  mockQueryDetail,
  mockQueryResult,
  setupMockFetch,
} from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("PlaygroundPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  function renderPlayground() {
    setupMockFetch([
      ...authRoutes(),
      ...metaRoutes(),
      {
        path: /\/releases\/v1\/queries\?/,
        method: "POST",
        response: { items: [{ result: mockQueryResult }] },
      },
      {
        path: `/releases/v1/queries/${mockQueryResult.query_id}`,
        response: mockQueryDetail,
      },
    ]);
    return renderWithProviders(
      <Routes>
        <Route path="/releases/:releaseTag/playground" element={<PlaygroundPage />} />
      </Routes>,
      { route: "/releases/v1/playground" },
    );
  }

  it("renders playground controls", () => {
    setupMockFetch([...authRoutes(), ...metaRoutes()]);
    renderWithProviders(
      <Routes>
        <Route path="/releases/:releaseTag/playground" element={<PlaygroundPage />} />
      </Routes>,
      { route: "/releases/v1/playground" },
    );

    expect(screen.getByText("Playground")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run query" })).toBeInTheDocument();
  });

  it("runs query and shows results timeline", async () => {
    renderPlayground();

    fireEvent.change(screen.getByPlaceholderText("Query text"), { target: { value: "hello" } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Run query" }));
    });

    expect(await screen.findByText("Timeline", { timeout: 3000 })).toBeInTheDocument();
    expect(screen.getByText("Semantic Results")).toBeInTheDocument();
    expect(screen.getAllByText(/Doc A/).length).toBeGreaterThan(0);
  });

  it("shows code snippet tabs", () => {
    setupMockFetch([...authRoutes(), ...metaRoutes()]);
    renderWithProviders(
      <Routes>
        <Route path="/releases/:releaseTag/playground" element={<PlaygroundPage />} />
      </Routes>,
      { route: "/releases/v1/playground" },
    );

    fireEvent.click(screen.getByRole("button", { name: "Python" }));
    expect(screen.getByLabelText("Copy snippet")).toBeInTheDocument();
    expect(screen.getByText(/import requests/)).toBeInTheDocument();
  });
});
