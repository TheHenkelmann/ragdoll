// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StageModal } from "./StageModal";
import { mockRelease, mockStage, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("StageModal", () => {
  const onClose = vi.fn();
  const onChanged = vi.fn();

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    onClose.mockReset();
    onChanged.mockReset();
  });

  it("returns null when closed", () => {
    setupMockFetch([{ path: "/stages", response: [mockStage] }]);
    const { container } = renderWithProviders(
      <StageModal
        open={false}
        onClose={onClose}
        releases={[mockRelease]}
        stages={[mockStage]}
        onChanged={onChanged}
      />,
      { routerProps: {} },
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("lists stages and creates a new one", async () => {
    setupMockFetch([
      { path: "/stages", response: [mockStage] },
      { path: "/releases", response: [mockRelease] },
      { path: "/stages", method: "POST", response: { ...mockStage, tag: "qa" } },
    ]);
    renderWithProviders(
      <StageModal
        open
        onClose={onClose}
        releases={[mockRelease]}
        stages={[mockStage]}
        onChanged={onChanged}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("dev")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByLabelText("Stage tag"), { target: { value: "qa" } });
    fireEvent.click(screen.getByRole("button", { name: "Create stage" }));

    await waitFor(() => {
      expect(onChanged).toHaveBeenCalled();
    });
  });

  it("deletes a stage", async () => {
    setupMockFetch([
      { path: "/stages", response: [mockStage] },
      { path: "/releases", response: [mockRelease] },
      { path: `/stages/${mockStage.tag}`, method: "DELETE", status: 204, response: undefined },
    ]);
    renderWithProviders(
      <StageModal
        open
        onClose={onClose}
        releases={[mockRelease]}
        stages={[mockStage]}
        onChanged={onChanged}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("dev")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    await waitFor(() => {
      expect(onChanged).toHaveBeenCalled();
    });
  });
});
