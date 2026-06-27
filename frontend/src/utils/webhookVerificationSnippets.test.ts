// SPDX-License-Identifier: AGPL-3.0-only

import { describe, expect, it } from "vitest";
import { webhookVerificationSnippet } from "./webhookVerificationSnippets";

describe("webhookVerificationSnippets", () => {
  it("embeds the secret when visible", () => {
    const snippet = webhookVerificationSnippet("python", "abc-123", true);
    expect(snippet).toContain('"abc-123"');
    expect(snippet).not.toContain("•");
  });

  it("masks the secret when hidden", () => {
    const snippet = webhookVerificationSnippet("node", "abc-123", false);
    expect(snippet).not.toContain("abc-123");
    expect(snippet).toContain("•");
  });

  it("includes timestamp.body signing input", () => {
    const snippet = webhookVerificationSnippet("rust", "secret", true);
    expect(snippet).toContain("{timestamp_header}.{body}");
  });
});
