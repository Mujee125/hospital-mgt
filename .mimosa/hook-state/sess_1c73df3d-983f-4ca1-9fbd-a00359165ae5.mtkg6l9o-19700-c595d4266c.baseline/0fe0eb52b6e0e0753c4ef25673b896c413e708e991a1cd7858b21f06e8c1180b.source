/* eslint-disable react-refresh/only-export-components */
/**
 * Auth context — exposes the active session to the whole tree so sidebar
 * items, guards, and the header can render role-appropriately without
 * prop-drilling. The session itself lives in Rust app state; `useMe` is the
 * bridge. `login`/`logout` mutate that state and refresh the query.
 */
import { createContext, useContext, useCallback, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import type { Session, Permission } from "./rbac";
import { can } from "./rbac";

interface AuthCtx {
  session: Session | null;
  isLoading: boolean;
  login: (username: string, password: string) => Promise<Session>;
  logout: () => Promise<void>;
  has: (perm: Permission) => boolean;
}

const Ctx = createContext<AuthCtx | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const qc = useQueryClient();
  const me = useQuery({
    queryKey: ["auth", "me"],
    queryFn: () => invoke<Session>("me"),
    retry: false,
    staleTime: 0,
  });

  const session = me.data ?? null;

  const login = useCallback(
    async (username: string, password: string) => {
      const s = await invoke<Session>("login", { request: { username, password } });
      await qc.invalidateQueries({ queryKey: ["auth", "me"] });
      return s;
    },
    [qc],
  );

  const logout = useCallback(async () => {
    try {
      await invoke("logout");
    } finally {
      qc.setQueryData(["auth", "me"], null);
      qc.clear();
    }
  }, [qc]);

  const has = useCallback(
    (perm: Permission) => can(session?.permissions, perm),
    [session],
  );

  return (
    <Ctx.Provider value={{ session, isLoading: me.isLoading, login, logout, has }}>
      {children}
    </Ctx.Provider>
  );
}

export function useAuth(): AuthCtx {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useAuth must be used within <AuthProvider>");
  return ctx;
}
