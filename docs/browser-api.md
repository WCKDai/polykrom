# Browser command API

After the browser app starts, `window.polykrom` controls the design open in that tab.
Calls use the same command ids, parameters, control handlers and undo history as the desktop
control channel. No separate engine or server is started.

| Call | Result |
|---|---|
| `polykrom.request(method, params?)` | Any [control method](control-protocol.md), such as `ui.inspect`, `document.inspect` or `engine.batch` |
| `polykrom.execute(command, params?)` | Run an engine command without opening its interactive dialog |
| `polykrom.commands()` | Discover commands, their parameters and current enablement |
| `polykrom.inspect(params?)` | Inspect the active document, optionally with `{measure: true}` |

Every call returns a promise. It resolves to the control reply's `result`, or rejects with an
`Error`. Omitted or `null` parameters become `{}`. Parameters must be JSON serializable and
at most 16 MiB of JSON. At most 256 requests may be pending; excess requests reject without
running. Requests run on the UI thread between frames. A hidden or suspended tab can delay
completion; keep it active while driving the app. A reload ends the session and its pending work.

In the browser's developer console, on an empty design:

```js
await polykrom.commands();
await polykrom.request("ui.home", {open: false});
await polykrom.execute("solid.box", {
  length: 30, width: 20, height: 10, body_name: "Browser API box"
});
await polykrom.inspect({measure: true});
await polykrom.execute("edit.undo");
await polykrom.execute("edit.redo");
await polykrom.request("ui.inspect");
```

An agent with JavaScript evaluation access to this tab can make the same calls. The API does
not register WebMCP tools or expose a network listener. Scripts executing in the page can use
the API with the page's browser permissions. File paths refer to the browser's virtual file
system, not arbitrary files on the user's computer. Native-only operations remain subject to
the browser host's limitations; this API does not grant desktop filesystem or window access.

## Verify a browser build

Build the app with `trunk build` in `apps/polykrom-web`, then run from the repository root:

```sh
cargo xtask web-api-test --site dist/web
```

The test starts a local server and headless Chrome, discovers commands, creates a box in the
live document, checks undo and redo, and verifies error handling and the pending-request limit.
It also checks that Ctrl+Tab and Ctrl+Shift+Tab are left to the browser, which switches tabs with them.
Set `CHROME` to a Chrome or Chromium executable if it is not installed in a usual location.
