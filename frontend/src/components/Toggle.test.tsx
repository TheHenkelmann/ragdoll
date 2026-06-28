// SPDX-License-Identifier: AGPL-3.0-only

import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Toggle } from "./Toggle";
import { renderWithProviders } from "../test/renderWithProviders";

describe("Toggle", () => {
  it("calls onChange when clicked", () => {
    const onChange = vi.fn();
    renderWithProviders(<Toggle checked={false} onChange={onChange} label="Enable feature" />);

    fireEvent.click(screen.getByRole("switch", { name: "Enable feature" }));
    expect(onChange).toHaveBeenCalledWith(true);
  });
});
