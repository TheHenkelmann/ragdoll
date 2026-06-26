// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ReleaseModal } from "./ReleaseModal";
import { mockRelease, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("ReleaseModal", () => {
  const onClose = vi.fn();
  const onChanged = vi.fn();
  const onSelect = vi.fn();

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    onClose.mockReset();
    onChanged.mockReset();
    onSelect.mockReset();
  });

  it("returns null when closed", () => {
    setupMockFetch([{ path: "/releases", response: [mockRelease] }]);
    const { container } = renderWithProviders(
      <ReleaseModal
        open={false}
        onClose={onClose}
        releases={[mockRelease]}
        currentTag="v1"
        tab="dashboard"
        onChanged={onChanged}
        onSelect={onSelect}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("lists releases and selects one", async () => {
    setupMockFetch([{ path: "/releases", response: [mockRelease] }]);
    renderWithProviders(
      <ReleaseModal
        open
        onClose={onClose}
        releases={[mockRelease]}
        currentTag="v1"
        tab="dashboard"
        onChanged={onChanged}
        onSelect={onSelect}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("v1")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("v1"));
    expect(onSelect).toHaveBeenCalledWith("v1");
    expect(onClose).toHaveBeenCalled();
  });

  it("creates a new release", async () => {
    setupMockFetch([
      { path: "/releases", response: [mockRelease] },
      {
        path: "/releases",
        method: "POST",
        response: { ...mockRelease, tag: "v2" },
      },
    ]);
    renderWithProviders(
      <ReleaseModal
        open
        onClose={onClose}
        releases={[mockRelease]}
        currentTag="v1"
        tab="dashboard"
        onChanged={onChanged}
        onSelect={onSelect}
      />,
    );

    fireEvent.change(screen.getByLabelText("Tag"), { target: { value: "v2" } });
    fireEvent.click(screen.getByRole("button", { name: "Create release" }));

    await waitFor(() => {
      expect(onChanged).toHaveBeenCalled();
      expect(onSelect).toHaveBeenCalledWith("v2");
    });
  });
});
