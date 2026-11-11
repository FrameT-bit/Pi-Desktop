# AI feedback — 4 prototypes

HTML prototypes of the Pi Desktop window for the update that adds turn feedback and
sessions working in parallel. Open **`index.html`** for the overview with screenshots,
or each file to see the animations.

| file | idea | where feedback lives |
|---|---|---|
| `pulso.html` | 01 · Pulse | message edge + receipt + header chip |
| `trilho.html` | 02 · Turn track | four-station block between messages |
| `atividade.html` | 03 · Activity line | strip above the composer + expandable log |
| `processos.html` | 04 · Sessions as processes | sidebar (run/wait/done) + approval strip |

`shots/` holds the static state for each at 1280×814 (the app's real chrome).
`app.css` holds the tokens, chrome, sidebar, composer, and the variants for each idea.

## Fidelity

- Tokens from the **Espresso Shot** flavor derived from `src/theme_catalog.rs` (page `#251a14`,
  canvas `#2f241f`, card `#382d28`, accent `#c98a4b`, ink `#f5efe7`).
- Real app fonts: JetBrains Mono (9–11px chrome, uppercase) and Pixelify Sans (15/1.65 prose).
- Real icons and logos from `resources/icons/ui` and `resources/logos`.
- App measurements: 62px header, 264px sidebar, 30px status, 12px gap, radius 0.
- The **simulate** button changes state without reloading; `?shot=1&static=1` freezes
  animations and hides the caption (used to generate the PNGs).

## What the backend must expose

All four concepts use the same per-session state; only the presentation changes.

```text
SessionRuntime {
  key, title, path,
  state: idle | running | waiting_approval | done_unread | error,
  phase: sent | thinking | tools | writing,     // track/pulse
  started_at, elapsed,
  current_tool: { name, target, started_at, output_tail },
  last_event: { tool, target, delta, at },
  queue_len, unread, last_error
}
```

RPC events that feed this state: `session_start`, `thinking_start/delta/end`,
`tool_call`, `tool_result`, `message_delta`, `done`, `error`, permission requests, and
`switch_session`. Today the app keeps **one** `RpcHandle` and switches sessions with
`switch_session`; true parallelism needs one process/RPC per running session and a
`HashMap<SessionKey, RpcHandle>` with every event tagged by session.

## Presentation rules (shared by all four)

- **One fact, one place.** The running-session count appears in the statusbar **or** in the
  sidebar summary — never both. The phase appears on the session row, not repeated in the chip.
- **Color = state.** `accent` = working/active; `warning` = waiting for you;
  `success` = done; `dim` = future/unknown. No motion under `prefers-reduced-motion`.
- **The active session** keeps a solid 2px bar; a background one uses the same bar breathing.
  A background session's tab gets the same state at a smaller scale.
- **No new space** for idea 01; ideas 02–04 add, respectively, a block between messages,
  a 30px strip, and a sidebar section.

## Next step

Pick a direction (the index recommends **01 + the sidebar from 04**) and implement
`SessionRuntime` before the drawing — the drawing depends on which events Pi actually emits
per session. The prototypes are static; no Rust code was changed by them.
