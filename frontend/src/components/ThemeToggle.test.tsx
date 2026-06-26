// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ThemeToggle } from "./ThemeToggle";
import { renderWithProviders } from "../test/renderWithProviders";

describe("ThemeToggle", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    document.documentElement.classList.remove("light");
  });

  it("toggles theme on click", () => {
    renderWithProviders(<ThemeToggle />, { token: null });

    expect(screen.getByLabelText("Switch to light mode")).toBeInTheDocument();
    fireEvent.click(screen.getByLabelText("Switch to light mode"));
    expect(screen.getByLabelText("Switch to dark mode")).toBeInTheDocument();
    expect(localStorage.getItem("ragdoll_theme")).toBe("light");
  });
});
