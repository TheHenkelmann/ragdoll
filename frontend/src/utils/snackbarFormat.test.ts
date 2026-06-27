// SPDX-License-Identifier: AGPL-3.0-only

import { describe, expect, it } from "vitest";
import { formatApiError, pushApiError } from "./snackbarFormat";

describe("formatApiError", () => {
  it("parses status-prefixed API errors", () => {
    expect(formatApiError("422 {\"detail\":\"Tag exists\"}")).toEqual({
      title: "Request failed (422)",
      body: '{"detail":"Tag exists"}',
    });
  });

  it("parses Error-wrapped API errors", () => {
    expect(formatApiError(new Error("401 unauthorized"))).toEqual({
      title: "Request failed (401)",
      body: "unauthorized",
    });
  });

  it("falls back to plain message", () => {
    expect(formatApiError("Something went wrong")).toEqual({
      title: "Something went wrong",
      body: "",
    });
  });
});

describe("pushApiError", () => {
  it("calls snackbar.error with parsed title and body", () => {
    const calls: Array<[string, string | undefined]> = [];
    pushApiError((title, body) => calls.push([title, body]), "500 server error");
    expect(calls).toEqual([["Request failed (500)", "server error"]]);
  });
});
