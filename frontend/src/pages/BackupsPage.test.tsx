// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BackupsPage } from "./BackupsPage";
import { authRoutes } from "../test/mockApi";
import { renderWithProviders } from "../test/renderWithProviders";

const mockListResponse = {
  backups: [
    {
      file_name: "ragdoll-20260627T093500Z-daily.db",
      trigger: "daily" as const,
      created_at: "2026-06-27T09:35:00Z",
      size_bytes: 2048,
    },
  ],
  retention: { keep_daily: 7, keep_manual: 10 },
};

vi.mock("../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/client")>();
  return {
    ...actual,
    listBackups: vi.fn(() => Promise.resolve(mockListResponse)),
    createBackup: vi.fn(() =>
      Promise.resolve({
        file_name: "ragdoll-20260627T101212Z-manual.db",
        trigger: "manual" as const,
        created_at: "2026-06-27T10:12:12Z",
        size_bytes: 4096,
      }),
    ),
    restoreBackup: vi.fn(() =>
      Promise.resolve({
        restored_from: "ragdoll-20260627T093500Z-daily.db",
        safety_backup: "ragdoll-20260627T101500Z-manual.db",
        restored_at: "2026-06-27T10:15:00Z",
      }),
    ),
    downloadBackup: vi.fn(() => Promise.resolve()),
    uploadBackup: vi.fn(() =>
      Promise.resolve({
        file_name: "ragdoll-20260627T102000123Z-manual.db",
        trigger: "manual" as const,
        created_at: "2026-06-27T10:20:00Z",
        size_bytes: 4096,
      }),
    ),
    deleteBackup: vi.fn(() =>
      Promise.resolve({
        deleted: true,
        file_name: "ragdoll-20260627T093500Z-daily.db",
      }),
    ),
  };
});

function renderPage() {
  return renderWithProviders(<BackupsPage />, {
    route: "/backups",
    mockRoutes: authRoutes(),
  });
}

describe("BackupsPage", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("renders backup list and retention info", async () => {
    renderPage();
    expect(await screen.findByText("ragdoll-20260627T093500Z-daily.db")).toBeInTheDocument();
    expect(screen.getByText(/Retention/i)).toBeInTheDocument();
    expect(screen.getByText(/RAGDOLL_BACKUP_KEEP_DAILY/i)).toBeInTheDocument();
  });

  it("creates backup on button click", async () => {
    const { createBackup, listBackups } = await import("../api/client");
    renderPage();
    await screen.findByText("ragdoll-20260627T093500Z-daily.db");

    fireEvent.click(screen.getAllByRole("button", { name: /create backup now/i })[0]);

    await waitFor(() => {
      expect(createBackup).toHaveBeenCalledTimes(1);
      expect(listBackups).toHaveBeenCalledTimes(2);
    });
  });

  it("requires confirmation before restore", async () => {
    const { restoreBackup } = await import("../api/client");
    renderPage();
    await screen.findByText("ragdoll-20260627T093500Z-daily.db");

    fireEvent.click(screen.getAllByRole("button", { name: /^restore$/i })[0]);
    expect(screen.getByText(/restore backup\?/i)).toBeInTheDocument();

    const confirm = screen.getByRole("button", { name: /restore database/i });
    expect(confirm).toBeDisabled();

    fireEvent.click(
      screen.getByRole("checkbox", {
        name: /I understand this will overwrite the live database/i,
      }),
    );
    expect(confirm).not.toBeDisabled();

    fireEvent.click(confirm);

    await waitFor(() => {
      expect(restoreBackup).toHaveBeenCalledWith("ragdoll-20260627T093500Z-daily.db", {
        safetyBackup: false,
      });
    });
  });

  it("requires typed file name before delete", async () => {
    const { deleteBackup } = await import("../api/client");
    renderPage();
    await screen.findByText("ragdoll-20260627T093500Z-daily.db");

    fireEvent.click(screen.getAllByRole("button", { name: /^delete$/i })[0]);
    const confirm = screen.getByRole("button", { name: /delete backup/i });
    expect(confirm).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText("ragdoll-20260627T093500Z-daily.db"), {
      target: { value: "ragdoll-20260627T093500Z-daily.db" },
    });
    expect(confirm).not.toBeDisabled();
    fireEvent.click(confirm);

    await waitFor(() => {
      expect(deleteBackup).toHaveBeenCalledWith("ragdoll-20260627T093500Z-daily.db");
    });
  });
});
