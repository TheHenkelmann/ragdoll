// SPDX-License-Identifier: AGPL-3.0-only

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SnackbarProvider, useSnackbar, useSnackbarInternal } from "./SnackbarContext";

function SnackbarProbe() {
  const snackbar = useSnackbar();
  const { items } = useSnackbarInternal();
  return (
    <div>
      <button type="button" onClick={() => snackbar.error("Load failed", "details")}>
        push-error
      </button>
      <button type="button" onClick={() => snackbar.error("Load failed", "details")}>
        push-duplicate
      </button>
      <button type="button" onClick={() => snackbar.success("Saved")}>
        push-success
      </button>
      <span data-testid="count">{items.length}</span>
      <span data-testid="merged-count">{items[0]?.count ?? 0}</span>
      <button type="button" onClick={() => items[0] && snackbar.dismiss(items[0].id)}>
        dismiss-first
      </button>
    </div>
  );
}

describe("SnackbarContext", () => {
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  beforeEach(() => {
    vi.stubGlobal(
      "requestAnimationFrame",
      (cb: FrameRequestCallback) => {
        return window.setTimeout(() => cb(performance.now()), 0) as unknown as number;
      },
    );
    vi.stubGlobal("cancelAnimationFrame", (id: number) => window.clearTimeout(id));
  });

  it("adds snackbar items via push helpers", () => {
    render(
      <SnackbarProvider>
        <SnackbarProbe />
      </SnackbarProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "push-error" }));
    expect(screen.getByText("Load failed")).toBeInTheDocument();
    expect(screen.getByTestId("count")).toHaveTextContent("1");
  });

  it("merges duplicate title and body", () => {
    render(
      <SnackbarProvider>
        <SnackbarProbe />
      </SnackbarProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "push-error" }));
    fireEvent.click(screen.getByRole("button", { name: "push-duplicate" }));
    expect(screen.getByTestId("count")).toHaveTextContent("1");
    expect(screen.getByTestId("merged-count")).toHaveTextContent("2");
    expect(screen.getByText(/×2/)).toBeInTheDocument();
  });

  it("dismisses items", () => {
    render(
      <SnackbarProvider>
        <SnackbarProbe />
      </SnackbarProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "push-success" }));
    expect(screen.getByText("Saved")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "dismiss-first" }));
    expect(screen.queryByText("Saved")).not.toBeInTheDocument();
  });

  it("removes expired items over time", async () => {
    vi.useFakeTimers();
    render(
      <SnackbarProvider>
        <SnackbarProbe />
      </SnackbarProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "push-success" }));
    expect(screen.getByText("Saved")).toBeInTheDocument();

    await act(async () => {
      vi.advanceTimersByTime(6000);
      await Promise.resolve();
    });

    expect(screen.queryByText("Saved")).not.toBeInTheDocument();
    vi.useRealTimers();
  });
});
