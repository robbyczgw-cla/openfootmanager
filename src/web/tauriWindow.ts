// Web replacement for `@tauri-apps/api/window`.
//
// The desktop app uses the Tauri window handle to intercept the close request
// (for "save before quit?") and to destroy the window on quit. In a browser
// there is no OS window to own, so:
//   * `onCloseRequested` maps to the `beforeunload` event. If the handler calls
//     `event.preventDefault()`, we trigger the browser's native "leave site?"
//     prompt (the closest equivalent — the app's own modal cannot block an
//     unload synchronously).
//   * `destroy` attempts `window.close()` (browsers only honour this for
//     script-opened tabs); otherwise it is a no-op.
//
// Aliased in for `@tauri-apps/api/window` by `vite.web.config.ts`.

export type UnlistenFn = () => void;

export interface CloseRequestedEvent {
  preventDefault(): void;
}

export interface WebAppWindow {
  onCloseRequested(
    handler: (event: CloseRequestedEvent) => void | Promise<void>,
  ): Promise<UnlistenFn>;
  destroy(): Promise<void>;
  close(): Promise<void>;
}

export function getCurrentWindow(): WebAppWindow {
  return {
    onCloseRequested(handler) {
      const listener = (browserEvent: BeforeUnloadEvent) => {
        let prevented = false;
        const event: CloseRequestedEvent = {
          preventDefault() {
            prevented = true;
          },
        };
        // The app's handler checks its dirty state synchronously before any
        // await, so `prevented` is set in time even though it may be async.
        void handler(event);
        if (prevented) {
          browserEvent.preventDefault();
          browserEvent.returnValue = "";
        }
      };
      window.addEventListener("beforeunload", listener);
      return Promise.resolve(() =>
        window.removeEventListener("beforeunload", listener),
      );
    },
    destroy() {
      window.close();
      return Promise.resolve();
    },
    close() {
      window.close();
      return Promise.resolve();
    },
  };
}
