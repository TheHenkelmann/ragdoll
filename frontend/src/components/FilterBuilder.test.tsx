// SPDX-License-Identifier: AGPL-3.0-only

import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { FilterBuilder } from "../components/FilterBuilder";

describe("FilterBuilder", () => {
  it("renders builder controls from JSON value", () => {
    render(
      <FilterBuilder
        value={JSON.stringify({ field: "meta.department", op: "eq", value: "hr" })}
        onChange={() => {}}
      />,
      { wrapper: MemoryRouter },
    );
    expect(screen.getByDisplayValue("meta.department")).toBeInTheDocument();
    expect(screen.getByDisplayValue("hr")).toBeInTheDocument();
  });
});
