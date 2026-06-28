// SPDX-License-Identifier: AGPL-3.0-only

import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { WebhookEventEditor } from "./WebhookEventEditor";
import { renderWithProviders } from "../test/renderWithProviders";

describe("WebhookEventEditor", () => {
  it("toggles individual ingest events", () => {
    const onChange = vi.fn();
    renderWithProviders(<WebhookEventEditor value={[]} onChange={onChange} />);

    fireEvent.click(screen.getByLabelText("Completed"));
    expect(onChange).toHaveBeenCalledWith(["completed"]);
  });
});
