// SPDX-License-Identifier: AGPL-3.0-only

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SnackbarProvider, useSnackbarInternal } from "../context/SnackbarContext";

function ExpandProbe() {
  const { push, items } = useSnackbarInternal();
  return (
    <div>
      <button
        type="button"
        onClick={() => push({ title: "Request failed", body: "422 detail", type: "error" })}
      >
        show
      </button>
      <span data-testid="remaining">{Math.round(items[0]?.remaining ?? 0)}</span>
    </div>
  );
}

describe("Snackbar", () => {
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  beforeEach(() => {
    vi.stubGlobal(
      "requestAnimationFrame",
      (cb: FrameRequestCallback) => {
        return window.setTimeout(() => cb(performance.now()), 16) as unknown as number;
      },
    );
    vi.stubGlobal("cancelAnimationFrame", (id: number) => window.clearTimeout(id));
  });

  it("expands body on expand hint click", () => {
    render(
      <SnackbarProvider>
        <ExpandProbe />
      </SnackbarProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "show" }));
    expect(screen.queryByText("422 detail")).not.toBeInTheDocument();

    const hint = document.querySelector(".snackbar-expand-hint");
    expect(hint).toBeTruthy();
    fireEvent.click(hint!);
    expect(screen.getByText("422 detail")).toBeInTheDocument();
  });

  it("expands body on header click", () => {
    render(
      <SnackbarProvider>
        <ExpandProbe />
      </SnackbarProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "show" }));
    expect(screen.queryByText("422 detail")).not.toBeInTheDocument();

    fireEvent.click(screen.getByText("Request failed"));
    expect(screen.getByText("422 detail")).toBeInTheDocument();
  });

  it("closes item via dismiss button", () => {
    render(
      <SnackbarProvider>
        <ExpandProbe />
      </SnackbarProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "show" }));
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText("Request failed")).not.toBeInTheDocument();
  });

  it("pauses countdown while hovering the container", async () => {
    vi.useFakeTimers();
    render(
      <SnackbarProvider>
        <ExpandProbe />
      </SnackbarProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "show" }));
    const container = document.querySelector(".snackbar-container");
    expect(container).toBeTruthy();

    await act(async () => {
      vi.advanceTimersByTime(2000);
      await Promise.resolve();
    });
    const remainingAfterTick = Number(screen.getByTestId("remaining").textContent);

    fireEvent.mouseEnter(container!);
    await act(async () => {
      vi.advanceTimersByTime(3000);
      await Promise.resolve();
    });
    expect(screen.getByTestId("remaining")).toHaveTextContent(String(remainingAfterTick));

    fireEvent.mouseLeave(container!);
    await act(async () => {
      vi.advanceTimersByTime(5000);
      await Promise.resolve();
    });
    expect(screen.queryByText("Request failed")).not.toBeInTheDocument();
  });
});
