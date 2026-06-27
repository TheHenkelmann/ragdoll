// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  CreateTagControl,
  DeleteConfirmDialog,
  EditableTag,
  ForkControl,
  ViewButton,
} from "./ObjectOverview";
import { renderWithProviders } from "../test/renderWithProviders";

describe("ObjectOverview components", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("ViewButton calls onClick", () => {
    const onClick = vi.fn();
    renderWithProviders(<ViewButton onClick={onClick} />);
    fireEvent.click(screen.getByRole("button", { name: "View" }));
    expect(onClick).toHaveBeenCalled();
  });

  it("CreateTagControl submits new tag", async () => {
    const onCreate = vi.fn().mockResolvedValue(undefined);
    renderWithProviders(<CreateTagControl label="Create" maxLength={12} onCreate={onCreate} />);

    fireEvent.click(screen.getByRole("button", { name: "Create" }));
    fireEvent.change(screen.getByPlaceholderText("tag"), { target: { value: "new-tag" } });
    fireEvent.click(screen.getByLabelText("Confirm"));

    await waitFor(() => {
      expect(onCreate).toHaveBeenCalledWith("new-tag");
    });
  });

  it("CreateTagControl shows validation error and blocks submit", () => {
    const onCreate = vi.fn();
    renderWithProviders(
      <CreateTagControl
        label="Create"
        maxLength={12}
        validate={(tag) => (tag === "taken" ? "Name already taken" : null)}
        onCreate={onCreate}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Create" }));
    fireEvent.change(screen.getByPlaceholderText("tag"), { target: { value: "taken" } });

    expect(screen.getByText("Name already taken")).toBeInTheDocument();
    expect(screen.getByLabelText("Confirm")).toBeDisabled();
    fireEvent.click(screen.getByLabelText("Confirm"));
    expect(onCreate).not.toHaveBeenCalled();
  });

  it("ForkControl submits fork tag", async () => {
    const onFork = vi.fn().mockResolvedValue(undefined);
    renderWithProviders(<ForkControl sourceTag="v1" maxLength={50} onFork={onFork} />);

    fireEvent.click(screen.getByRole("button", { name: "Fork" }));
    fireEvent.click(screen.getByLabelText("Confirm"));

    await waitFor(() => {
      expect(onFork).toHaveBeenCalledWith("v1-fork");
    });
  });

  it("EditableTag renames on submit", async () => {
    const onRename = vi.fn().mockResolvedValue(undefined);
    renderWithProviders(<EditableTag tag="v1" maxLength={50} onRename={onRename} />);

    fireEvent.click(screen.getByLabelText("Edit tag"));
    fireEvent.change(screen.getByDisplayValue("v1"), { target: { value: "v2" } });
    fireEvent.click(screen.getByLabelText("Confirm"));

    await waitFor(() => {
      expect(onRename).toHaveBeenCalledWith("v2");
    });
  });

  it("DeleteConfirmDialog requires exact confirmation text", async () => {
    const onConfirm = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    renderWithProviders(
      <DeleteConfirmDialog
        open
        typeLabel="release"
        tag="v1"
        onClose={onClose}
        onConfirm={onConfirm}
      />,
    );

    const deleteBtn = screen.getByRole("button", { name: "Delete" });
    expect(deleteBtn).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText("release/v1"), {
      target: { value: "release/v1" },
    });
    fireEvent.click(deleteBtn);

    await waitFor(() => {
      expect(onConfirm).toHaveBeenCalled();
      expect(onClose).toHaveBeenCalled();
    });
  });
});
