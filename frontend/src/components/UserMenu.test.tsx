// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { UserMenu } from "./UserMenu";
import { authRoutes, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";
import { getToken } from "../api/client";

describe("UserMenu", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("shows email initial and opens menu", async () => {
    setupMockFetch(authRoutes());
    renderWithProviders(<UserMenu />);

    await waitFor(() => {
      expect(screen.getByLabelText("Account menu")).toHaveTextContent("A");
    });
    fireEvent.click(screen.getByLabelText("Account menu"));

    expect(screen.getByText("admin@ragdoll.ai")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Logout" })).toBeInTheDocument();
  });

  it("logs out when logout is clicked", () => {
    setupMockFetch(authRoutes());
    renderWithProviders(<UserMenu />);

    fireEvent.click(screen.getByLabelText("Account menu"));
    fireEvent.click(screen.getByRole("button", { name: "Logout" }));

    expect(getToken()).toBeNull();
  });

  it("closes menu on outside click", async () => {
    setupMockFetch(authRoutes());
    renderWithProviders(
      <div>
        <UserMenu />
        <button type="button">Outside</button>
      </div>,
    );

    await waitFor(() => {
      expect(screen.getByLabelText("Account menu")).toHaveTextContent("A");
    });

    fireEvent.click(screen.getByLabelText("Account menu"));
    expect(screen.getByText("admin@ragdoll.ai")).toBeInTheDocument();

    fireEvent.mouseDown(screen.getByRole("button", { name: "Outside" }));
    expect(screen.queryByText("admin@ragdoll.ai")).not.toBeInTheDocument();
  });
});
