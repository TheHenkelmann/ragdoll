// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { UsersPage } from "./UsersPage";
import { authRoutes, setupMockFetch } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

const mockUser = {
  id: "user-1",
  email: "editor@ragdoll.ai",
  is_superadmin: false,
  permissions: ["releases:read", "sources:read"],
  created_at: "2026-01-01T00:00:00Z",
};

describe("UsersPage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("renders non-superadmin users", async () => {
    setupMockFetch([...authRoutes(), { path: "/users", response: [mockUser] }]);
    renderWithProviders(<UsersPage />);

    expect(await screen.findByText("editor@ragdoll.ai")).toBeInTheDocument();
    expect(screen.getByText(/2 permissions/)).toBeInTheDocument();
  });

  it("rejects weak passwords on create", async () => {
    setupMockFetch([...authRoutes(), { path: "/users", response: [mockUser] }]);
    renderWithProviders(<UsersPage />);

    await waitFor(() => {
      expect(screen.getByText("editor@ragdoll.ai")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Create user" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Email" }), {
      target: { value: "new@ragdoll.ai" },
    });
    fireEvent.change(screen.getByLabelText("Password"), {
      target: { value: "weak" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Edit permissions/i }));
    fireEvent.click(screen.getByRole("checkbox", { name: /sources:read/ }));
    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    await waitFor(() => {
      expect(
        screen.getByText("Password does not meet the strength requirements"),
      ).toBeInTheDocument();
    });
  });

  it("opens edit dialog for a regular user", async () => {
    setupMockFetch([...authRoutes(), { path: "/users", response: [mockUser] }]);
    renderWithProviders(<UsersPage />);

    expect(await screen.findByText("editor@ragdoll.ai")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));

    expect(await screen.findByText("Edit user")).toBeInTheDocument();
    expect(screen.getByDisplayValue("editor@ragdoll.ai")).toBeInTheDocument();
  });
});
