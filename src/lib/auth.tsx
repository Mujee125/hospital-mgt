/* eslint-disable react-refresh/only-export-components */
/**
 * Auth context — exposes the active session to the whole tree so sidebar
 * items, guards, and the header can render role-appropriately without
 * prop-drilling. The session itself lives in Rust app state; `useMe` is the
 * bridge. `login`/`logout` mutate that state and refresh the query.
 *
 * session_invalidated listener (wired 2026-09-05, closing the last UX gap
 * from the handoff §13.5): the backend has always EMITTED this event when
 * an admin resets a password / changes roles / a second login displaced the
 * session — but nothing listened, so users hit a raw error on their next
 * command. The listener clears the React-Query session cache and shows a
 * clean "signed out elsewhere" toast instead. Enforcement was never
 * affected (require_strong rejects server-side regardless); this is UX.
 */
import { createContext, useContext, useCallback, useEffect, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
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

  // session_invalidated: emitted by update_user (role change / deactivate)
  // and reset_user_password — and also by every login. Tauri events are
  // PER-PROCESS (a login on PC-B never reaches PC-A's webview), so the only
  // events this listener sees are from THIS process: either the user's own
  // login echo (payload user_id == mine, but the new token is valid — must
  // NOT sign out) or an admin action that targeted THIS account (own role
  // change / own password reset — the session rows were swept server-side,
  // so `me` genuinely fails). Discriminate by probing `me` before clearing.
  useEffect(() => {
    let un: (() => void) | undefined;
    let mounted = true;
    listen<{ user_id: number }>("session_invalidated", async (e) => {
      if (!mounted) return;
      const currentId = qc.getQueryData<Session | null>(["auth", "me"])?.user?.id;
      // No local session, or the event concerns a different user (an admin
      // acting on someone else from this PC): nothing to do locally.
      if (currentId === undefined || currentId === null) return;
      if (e.payload.user_id !== currentId) return;
      // Targeted at us. Own-login echo? Probe the session server-side.
      try {
        await invoke<Session>("me");
        // Still valid → this was our own login echo; do nothing.
      } catch {
        // Truly invalidated (own role change / own password reset swept the
        // session rows). Clear local state with a clean message.
        qc.setQueryData(["auth", "me"], null);
        qc.clear();
        toast.info("Your session has ended — please sign in again.", {
          duration: 6000,
        });
      }
    }).then((f) => {
      un = f;
    });
    return () => {
      mounted = false;
      un?.();
    };
  }, [qc]);

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
