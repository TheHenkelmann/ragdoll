// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProfilePage } from "./ProfilePage";
import { authRoutes } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

describe("ProfilePage", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("shows env var notice for superadmin", async () => {
    renderWithProviders(<ProfilePage />, {
      mockRoutes: authRoutes({ is_superadmin: true }),
    });
    expect(await screen.findByText(/RAGDOLL_SUPERADMIN_PW/)).toBeInTheDocument();
    expect(screen.queryByLabelText("New password")).not.toBeInTheDocument();
  });

  it("shows password fields for normal user", async () => {
    renderWithProviders(<ProfilePage />, {
      mockRoutes: authRoutes({ is_superadmin: false }),
    });
    expect(await screen.findByLabelText("New password")).toBeInTheDocument();
  });
});
