// Web replacement for `@tauri-apps/api/core`.
//
// In the browser build there is no Tauri runtime, so `invoke` cannot call into
// a native command. Instead it POSTs to the OpenFootManager web server's
// `/api/invoke/{command}` endpoint, preserving Tauri's `invoke` contract:
//   * resolves with the command's return value, and
//   * rejects with the backend error key (a `be.error.*` string), so the
//     existing `resolveBackendError` / i18n handling keeps working unchanged.
//
// This module is aliased in for `@tauri-apps/api/core` by `vite.web.config.ts`,
// which is why no application source needs to change for the web build.

export type InvokeArgs = Record<string, unknown>;

export async function invoke<T = unknown>(
  command: string,
  args?: InvokeArgs,
): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`/api/invoke/${command}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(args ?? {}),
    });
  } catch {
    // Network/server unreachable — surface a backend-style key.
    throw "be.error.web.serverUnreachable";
  }

  const raw = await response.text();
  let payload: unknown = undefined;
  if (raw.length > 0) {
    try {
      payload = JSON.parse(raw);
    } catch {
      payload = raw;
    }
  }

  if (!response.ok) {
    if (payload && typeof payload === "object" && "error" in payload) {
      throw (payload as { error: unknown }).error;
    }
    throw payload ?? `be.error.web.http.${response.status}`;
  }

  return payload as T;
}

// Some Tauri callers import `convertFileSrc`; provide a passthrough so any
// incidental usage does not break the web build.
export function convertFileSrc(filePath: string): string {
  return filePath;
}
