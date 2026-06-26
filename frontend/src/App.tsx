// SPDX-License-Identifier: AGPL-3.0-only

import { Navigate, Route, Routes, useLocation } from "react-router-dom";
import { Layout } from "./components/Layout";
import { AuthProvider, useAuth } from "./context/AuthContext";
import { ThemeProvider } from "./context/ThemeContext";
import { DashboardPage } from "./pages/DashboardPage";
import { DatabasePage } from "./pages/DatabasePage";
import { LoginPage } from "./pages/LoginPage";
import { NotFoundPage } from "./pages/NotFoundPage";
import { PlaygroundPage } from "./pages/PlaygroundPage";
import { ReleasesOverviewPage } from "./pages/ReleasesOverviewPage";
import { SettingsPage } from "./pages/SettingsPage";
import { SourcesPage } from "./pages/SourcesPage";
import { StagesOverviewPage } from "./pages/StagesOverviewPage";

function Protected({ children }: { children: React.ReactNode }) {
  const { token } = useAuth();
  const location = useLocation();
  if (!token) {
    const returnTo = encodeURIComponent(location.pathname + location.search);
    return <Navigate to={`/login?redirect=${returnTo}`} replace />;
  }
  return <>{children}</>;
}

export function App() {
  return (
    <ThemeProvider>
      <AuthProvider>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route
            path="/"
            element={
              <Protected>
                <Layout />
              </Protected>
            }
          >
            <Route index element={<Navigate to="/releases" replace />} />
            <Route path="releases" element={<ReleasesOverviewPage />} />
            <Route path="stages" element={<StagesOverviewPage />} />
            <Route path="releases/:releaseTag" element={<DashboardPage />} />
            <Route path="releases/:releaseTag/dashboard" element={<Navigate to=".." replace relative="path" />} />
            <Route path="releases/:releaseTag/playground" element={<PlaygroundPage />} />
            <Route path="releases/:releaseTag/sources" element={<SourcesPage />} />
            <Route path="releases/:releaseTag/database" element={<DatabasePage />} />
            <Route path="releases/:releaseTag/settings" element={<SettingsPage />} />
            <Route path="stages/:stageTag" element={<DashboardPage />} />
            <Route
              path="stages/:stageTag/dashboard"
              element={<Navigate to=".." replace relative="path" />}
            />
          </Route>
          <Route
            path="*"
            element={
              <Protected>
                <NotFoundPage />
              </Protected>
            }
          />
        </Routes>
      </AuthProvider>
    </ThemeProvider>
  );
}
