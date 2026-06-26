// SPDX-License-Identifier: AGPL-3.0-only

import { Link } from "react-router-dom";

export function NotFoundPage() {
  return (
    <div className="flex min-h-screen flex-col items-center justify-center p-6">
      <div className="card w-full max-w-md space-y-4 text-center">
        <div className="flex items-center justify-center gap-3">
          <img src="/assets/logo.png" alt="" className="h-10 w-auto" aria-hidden />
          <span className="text-2xl font-semibold">Ragdoll</span>
        </div>
        <h2 className="text-xl font-semibold">Page not found</h2>
        <p className="text-sm text-[var(--muted)]">
          The URL you requested does not match any page in this app.
        </p>
        <Link to="/releases" className="btn-primary inline-block">
          Go to releases
        </Link>
      </div>
    </div>
  );
}
