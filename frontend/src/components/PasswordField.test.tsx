// SPDX-License-Identifier: AGPL-3.0-only

import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { PasswordField } from "./PasswordField";
import { renderWithProviders } from "../test/renderWithProviders";
import { evaluatePassword, generatePassword } from "../utils/password";

function Harness({ showStrength = false, showGenerate = false }: { showStrength?: boolean; showGenerate?: boolean }) {
  const [value, setValue] = useState("");
  return (
    <PasswordField
      label="Password"
      value={value}
      onChange={setValue}
      showStrength={showStrength}
      showGenerate={showGenerate}
    />
  );
}

describe("PasswordField", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("toggles visibility", () => {
    renderWithProviders(<Harness />, { mockRoutes: [] });
    const input = screen.getByLabelText("Password") as HTMLInputElement;
    expect(input.type).toBe("password");
    fireEvent.click(screen.getByLabelText("Show password"));
    expect((screen.getByLabelText("Password") as HTMLInputElement).type).toBe("text");
  });

  it("generate fills a valid password", () => {
    renderWithProviders(<Harness showGenerate showStrength />);
    fireEvent.click(screen.getByRole("button", { name: "Generate strong password" }));
    const input = screen.getByLabelText("Password") as HTMLInputElement;
    expect(evaluatePassword(input.value).isValid).toBe(true);
  });
});

describe("password utils", () => {
  it("generatePassword meets policy", () => {
    expect(evaluatePassword(generatePassword()).isValid).toBe(true);
  });
});
