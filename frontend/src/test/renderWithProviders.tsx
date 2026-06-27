// SPDX-License-Identifier: AGPL-3.0-only

import { render, type RenderOptions } from "@testing-library/react";
import type { ReactElement, ReactNode } from "react";
import { MemoryRouter, type MemoryRouterProps } from "react-router-dom";
import { setToken } from "../api/client";
import { AuthProvider } from "../context/AuthContext";
import { SnackbarProvider } from "../context/SnackbarContext";
import { ThemeProvider } from "../context/ThemeContext";
import { setupMockFetch, type MockRoute } from "./mockApi";

export type RenderWithProvidersOptions = {
  route?: string;
  routerProps?: Omit<MemoryRouterProps, "initialEntries">;
  token?: string | null;
  mockRoutes?: MockRoute[];
} & Omit<RenderOptions, "wrapper">;

export function renderWithProviders(ui: ReactElement, options: RenderWithProvidersOptions = {}) {
  const { route = "/", routerProps, token = "test-token", mockRoutes, ...renderOptions } = options;

  localStorage.clear();
  if (token) setToken(token);
  else setToken(null);

  if (mockRoutes) {
    setupMockFetch(mockRoutes);
  }

  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <ThemeProvider>
        <SnackbarProvider>
          <AuthProvider>
            <MemoryRouter initialEntries={[route]} {...routerProps}>
              {children}
            </MemoryRouter>
          </AuthProvider>
        </SnackbarProvider>
      </ThemeProvider>
    );
  }

  return render(ui, { wrapper: Wrapper, ...renderOptions });
}
