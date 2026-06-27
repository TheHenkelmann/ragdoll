// SPDX-License-Identifier: AGPL-3.0-only

import { useCallback, useMemo } from "react";
import { useAuth } from "../context/AuthContext";

export function usePermissions() {
  const { status } = useAuth();
  const ready = status !== null;
  const isSuperadmin = status?.is_superadmin ?? false;
  const permissions = status?.permissions ?? [];

  const can = useCallback(
    (permission: string) => {
      if (!ready) return false;
      if (isSuperadmin) return true;
      return permissions.includes(permission);
    },
    [ready, isSuperadmin, permissions],
  );

  const canAny = useCallback(
    (...required: string[]) => {
      if (!ready) return false;
      if (isSuperadmin) return true;
      return required.some((p) => permissions.includes(p));
    },
    [ready, isSuperadmin, permissions],
  );

  return useMemo(
    () => ({ can, canAny, ready, isSuperadmin, permissions }),
    [can, canAny, ready, isSuperadmin, permissions],
  );
}
