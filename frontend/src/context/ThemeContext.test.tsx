// SPDX-License-Identifier: AGPL-3.0-only

import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ThemeProvider, useTheme } from "./ThemeContext";

function ThemeProbe() {
  const { theme, toggle } = useTheme();
  return (
    <div>
      <span data-testid="theme">{theme}</span>
      <button type="button" onClick={toggle}>
        toggle
      </button>
    </div>
  );
}

describe("ThemeContext", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    localStorage.clear();
    document.documentElement.classList.remove("light");
  });

  it("defaults to dark theme", () => {
    render(
      <ThemeProvider>
        <ThemeProbe />
      </ThemeProvider>,
    );
    expect(screen.getByTestId("theme")).toHaveTextContent("dark");
    expect(document.documentElement.classList.contains("light")).toBe(false);
  });

  it("restores theme from localStorage", () => {
    localStorage.setItem("ragdoll_theme", "light");
    render(
      <ThemeProvider>
        <ThemeProbe />
      </ThemeProvider>,
    );
    expect(screen.getByTestId("theme")).toHaveTextContent("light");
    expect(document.documentElement.classList.contains("light")).toBe(true);
  });

  it("toggle switches theme and persists choice", () => {
    render(
      <ThemeProvider>
        <ThemeProbe />
      </ThemeProvider>,
    );

    act(() => {
      screen.getByText("toggle").click();
    });

    expect(screen.getByTestId("theme")).toHaveTextContent("light");
    expect(localStorage.getItem("ragdoll_theme")).toBe("light");
    expect(document.documentElement.classList.contains("light")).toBe(true);

    act(() => {
      screen.getByText("toggle").click();
    });

    expect(screen.getByTestId("theme")).toHaveTextContent("dark");
    expect(localStorage.getItem("ragdoll_theme")).toBe("dark");
  });

  it("throws when useTheme is used outside provider", () => {
    expect(() => render(<ThemeProbe />)).toThrow("ThemeProvider missing");
  });
});
