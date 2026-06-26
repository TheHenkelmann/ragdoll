// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NotFoundPage } from "./NotFoundPage";
import { renderWithProviders } from "../test/renderWithProviders";

describe("NotFoundPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("renders not found message and link to releases", () => {
    renderWithProviders(<NotFoundPage />, { route: "/missing" });

    expect(screen.getByText("Page not found")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Go to releases" })).toHaveAttribute("href", "/releases");
  });
});
