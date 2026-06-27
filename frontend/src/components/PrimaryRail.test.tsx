// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PrimaryRail } from "./PrimaryRail";
import { authRoutes, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("PrimaryRail", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("collapses expanded rail after navigation click until mouse leaves", () => {
    setupMockFetch(authRoutes());
    renderWithProviders(<PrimaryRail />);
    const shell = document.querySelector(".rail-shell");
    expect(shell).toBeTruthy();

    fireEvent.click(screen.getByRole("link", { name: "Stages" }));
    expect(shell!.className).toContain("rail-collapsed");

    fireEvent.mouseLeave(shell!);
    expect(shell!.className).not.toContain("rail-collapsed");
  });
});
