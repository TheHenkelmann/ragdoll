// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { InfoTip } from "./InfoTip";
import { renderWithProviders } from "../test/renderWithProviders";

describe("InfoTip", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("shows tooltip on hover and click", () => {
    renderWithProviders(<InfoTip text="Helpful info" />);

    const button = screen.getByLabelText("More info");
    expect(screen.queryByText("Helpful info")).not.toBeInTheDocument();

    fireEvent.mouseEnter(button);
    expect(screen.getByText("Helpful info")).toBeInTheDocument();

    fireEvent.mouseLeave(button);
    expect(screen.queryByText("Helpful info")).not.toBeInTheDocument();

    fireEvent.click(button);
    expect(screen.getByText("Helpful info")).toBeInTheDocument();
  });

  it("renders danger tone styling", () => {
    renderWithProviders(<InfoTip text="Warning" tone="danger" wide />);

    fireEvent.click(screen.getByLabelText("More info"));
    expect(screen.getByText("Warning")).toHaveClass("w-80");
  });
});
