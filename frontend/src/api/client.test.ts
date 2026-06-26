// SPDX-License-Identifier: AGPL-3.0-only

import { beforeEach, describe, expect, it, vi } from "vitest";
import { API_PREFIX, api, getToken, publicApi, setToken } from "./client";

describe("api client", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("prefixes API paths", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );

    await publicApi("/health");
    expect(fetchMock).toHaveBeenCalledWith(`${API_PREFIX}/health`, expect.any(Object));
  });

  it("stores and clears auth tokens", () => {
    setToken("abc123");
    expect(getToken()).toBe("abc123");
    setToken(null);
    expect(getToken()).toBeNull();
  });

  it("returns undefined for 204 responses", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(null, { status: 204 }));
    await expect(publicApi("/noop", { method: "DELETE" })).resolves.toBeUndefined();
  });

  it("sends bearer token for authenticated requests", async () => {
    setToken("secret-token");
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );

    await api("/releases");
    expect(fetchMock).toHaveBeenCalledWith(
      `${API_PREFIX}/releases`,
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Bearer secret-token" }),
      }),
    );
  });

  it("clears token and redirects on 401", async () => {
    setToken("expired");
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("unauthorized", { status: 401 }));
    const assign = vi.fn();
    Object.defineProperty(window, "location", {
      configurable: true,
      value: { ...window.location, assign },
    });

    await expect(api("/releases")).rejects.toThrow("unauthorized");
    expect(getToken()).toBeNull();
    expect(assign).toHaveBeenCalled();
  });
});
