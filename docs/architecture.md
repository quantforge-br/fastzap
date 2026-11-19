# FastZap Architecture

## Overview

FastZap is a Tauri v2 application. The Rust process (`src-tauri/`) is the authority
over state, preferences, accounts, secrets, and native integrations. The local
UI is a SvelteKit SPA (`src/`, `adapter-static`) loaded in the `ui` webview
of the `main` window. Each WhatsApp Web account is a separate **child webview**,
loading `https://web.whatsapp.com/`, positioned inside the same window.

```text
src-tauri/src/main.rs
        |
        v
lib.rs::run()               plugins, setup, lifecycle events
        |
        +--> app/            CLI, single instance, D-Bus, signals, window, shortcuts
        +--> settings/       typed domains + versioned JSON store
        +--> migration/      one-shot ZapZap import
        +--> accounts/       accounts (SQLite) and avatars
        +--> webview/        child webviews, injected scripts, remote bridge
        +--> evolution/      REST client, WebSocket, message store
        +--> notifications/  policy + per-platform backends
        +--> tray/ downloads/ permissions/ proxy/ customizations/
             spellcheck/ reporting/ update/ autostart/ i18n/
        |
        v
commands/                   IPC surface invoked by the frontend
```

Dependency rules:

- `settings`, `secrets`, `paths`, and `i18n` do not depend on any other
  feature module.
- Features depend on `settings`/`accounts`, never directly on each other;
  coordination happens in `app::state::AppState` or via events.
- `commands/` is the only layer that knows the IPC call format.

## Shared state

`app::state::AppState` is registered with `app.manage()` and holds:

- `SettingsStore` (`RwLock<Settings>` + JSON path);
- `AccountStore` (SQLite `accounts.db`);
- `UnreadRegistry` (per-account count → tray, badge, and sidebar);
- `DownloadHistory`.

`evolution::Evolution` is registered separately (`app.manage`) and holds one
`evolution::Session` per active Evolution Go account.

UI-relevant state changes are emitted as Tauri events with stable names
(`settings://changed`, `accounts://changed`, `unread://changed`,
`evolution://event`, `downloads://changed`).
## Main window and WhatsApp Web accounts

The `main` window is created in `app::window` with a full-screen local webview
(sidebar, overlays, settings, native chat). For each enabled WhatsApp Web
account, `webview::manager` creates a child webview with `Window::add_child`
(Tauri `unstable` feature):

- dedicated `data_directory` at `<app_data>/profiles/<account-id>` (Linux and
  Windows) or a `data_store_identifier` derived from the ID on macOS. The bundle
  requires macOS 14 (`bundle.macOS.minimumSystemVersion`), because earlier
  versions ignore the identifier and the proxy (`macos-proxy`) and would share
  the session across accounts;
- Chrome `user_agent` (WhatsApp Web rejects plain WebKit) or the per-account
  User-Agent;
- `initialization_script` with the bridge (`webview/scripts/bridge.js`) and the
  optional scripts ported from ZapZap;
- `on_navigation`/`on_new_window` enforcing the host allowlist. As in ZapZap,
  `on_navigation` only blocks outside hosts (WhatsApp loads background frames
  such as `flows.whatsapp.net`); user-opened links arrive via `on_new_window`
  and go to the browser. Popups for allowed hosts open in a related `popup-<n>`
  window, with the same allowlist and no command access;
- `on_download` forwarding to `downloads` (on Linux only the destination; the
  completion comes from `downloads::linux`);
- `proxy_url` when the global proxy is active.

On Windows and macOS, Tauri's drag-and-drop handler is disabled on `wa-*`
accounts so WhatsApp Web receives files through the native HTML mechanism.
WebKitGTK passes OS file drops to the page without `File` objects
(`dataTransfer.files` is empty, only `text/uri-list` arrives), so on Linux the
handler stays on and `webview/file_drop.rs` takes the dropped paths, reads
each file (`upload::read_upload`, 100 MB cap), pushes it into the page in
base64 chunks through `eval`, and `__FASTZAP__.dropEnd` in `bridge.js`
dispatches the `drop` at the drop point with real files. WebKit pins a
constructed `DataTransfer`'s `effectAllowed`/`dropEffect` to `none`, which
WhatsApp rejects, so the replay reports a copy. The hover reaches the page
as `text/uri-list` without `Files`, so `bridge.js` intercepts those native
`dragenter`/`dragover` events and mirrors them with a placeholder file,
which makes WhatsApp show its drop zone during the drag; wry's `Leave` event
(a drag that leaves without dropping) calls `__FASTZAP__.dragLeave`. No
bridge command is involved. The `ui` webview keeps Tauri events for Evolution
Go chat attachments.

The frontend reports the free area (right of the sidebar) via
`layout_set_bounds`; the manager positions the active account in that area and
hides the others. Grid mode is ZapZap's accounts overview: all child webviews
stay hidden and the local UI (`AccountsOverview.svelte`) shows one card per
account, WhatsApp Web or Evolution Go, including disabled ones, in `grid_cols`
columns; the card activates the account (a disabled one asks before
re-activating). When a local screen must cover the content (settings, dialogs),
the frontend calls `layout_set_overlay(true)` and the child webviews are
hidden. With grid or overlay, keyboard focus goes to the `ui` webview.

On Linux, Tauri stacks child webviews in a vertical `GtkBox` and ignores
`set_position`. `webview::stage` inserts a `GtkOverlay` into the window: the
`ui` webview is the main child (filling the whole window) and each account is
an overlay child positioned via margins and `size_request`.

If an account's web process ends unrequested (crash or memory limit), the
`web-process-terminated` signal reloads the page after 1 s, like ZapZap's
`_on_render_crash`; without it the account stayed blank until the webview was
recreated. Linux only.

### Remote bridge

`build.rs` declares every app command in an `AppManifest`, so Tauri's ACL
requires an explicit permission for each one. `capabilities/default.json`
grants the local commands only to the `ui` webview;
`capabilities/whatsapp-remote.json` lets `wa-*` webviews at
`https://web.whatsapp.com/*` invoke only the `wa_*` commands in
`commands::bridge` (title, shortcuts, theme events, account switcher, and
profile photo; notifications join with the desktop-integration phase). The
calling account is always derived from the webview label, never from
arguments. `wa_shortcut` only accepts the shortcuts listed in `REMOTE_ACTIONS`
and applies them to the calling account; paste-as-plain-text reads the
clipboard, so on Linux it is handled in WebKitGTK's native `key-press-event`
and on other platforms it requires the focused window. `wa_profile_photo` only
downloads from `*.whatsapp.net`, without following redirects and with a byte
cap. The built-in switcher opens a native menu with the other enabled
accounts. Settings captured at webview creation (init scripts and proxy)
recreate the accounts when they change. The bridge script replaces
`window.Notification` and watches `document.title` to extract the unread
count, as ZapZap did via the page title.

### Quick-access panel

`app::panel` creates the hidden `panel` window at startup (`/panel` route,
fixed 320 px width, off the taskbar) and decides visibility like ZapZap:
`always`, `when_hidden` (main window hidden or minimized), or `on_demand`
(only via `show_panel`, from the tray). Closing dismisses the panel until the
mode or the main window state changes. `window::show` and `window::hide`
report the main window visibility, because tao queues the requests and
`is_visible` only reflects the change afterwards. Off Wayland, the default
position is the desktop's top-right corner (−15/+16), it is remembered after
500 ms without movement and, with `lock_position`, restored on every `Moved`.
The page uses the same account, unread, layout, and preference events as the
main window, and the `default` capability covers the `ui` and `panel`
webviews.

### Client-side decorations (CSR)

With `appearance.title_bar` off there is no title bar: neither native
decoration nor `TitleBar.svelte` (`window::native_decorations`), useful in
tiling window managers.

With `appearance.csr`, the window loses its native decorations
(`set_decorations`) and the UI draws `TitleBar.svelte`: dragging and double
click via `data-tauri-drag-region`, buttons with ZapZap's themes (an empty or
unknown theme follows `XDG_CURRENT_DESKTOP`, read by `app_desktop`) and, when
not maximized, a 4 px frame with `startResizeDragging` edges. Because the UI
reports the content-area bounds, account webviews move below the bar with no
Rust-side math.

### Theme

`app::theme` resolves the effective scheme. In `auto`, the
`org.freedesktop.appearance color-scheme` key of the Settings portal wins when
it states a preference, with `SettingChanged` watched (`monitor_system`);
without a preference, the window theme reported by the toolkit applies
(`WindowEvent::ThemeChanged`). The theme is applied to the window (decorations
and the UI's `prefers-color-scheme`) and to pages via `theme_controller.js`.

## Settings and menu bar

`src/lib/stores/settings.svelte.ts` keeps a reactive copy of the preferences
(`settings_get` + `settings-changed` event) and saves with `settings_patch`.
`src/lib/settings/` holds the navigation (`pages.ts`, in ZapZap's order),
`SettingsView.svelte`, and one page per domain in `pages/`; shared controls
live in `src/lib/components/settings/`. Preferences read only at startup show
`RestartBar`, which invokes the `restart` action. `commands::app` exposes build
information (`build_info`, filled by `FASTZAP_BUILD_*`), the proxy password in
the keyring, defaults restoration, file/folder dialogs, and path opening.

The Accounts page and the sidebar context menu use `src/lib/accounts/`.
Icons follow ZapZap's format (`accounts::icon` / `icon.ts`): generated SVG
with random colors, a 256 px PNG photo cropped to a circle, or the
`zapzap-account-image:v1:` profile holding both choices. The chosen photo goes
through `account_photo_pick`, which opens the dialog and normalizes the image
in Rust, without exposing the path to the UI. Account 1 (`storage-whats` in
ZapZap) cannot be removed.

`MenuBar.svelte` reproduces the File/View/Chat/Users/Help menus with native
menus (`@tauri-apps/api/menu`), because HTML menus would render under the
accounts' native webviews. The interface scale becomes zoom of the `ui` webview
and multiplies each account's zoom; the UI converts the bounds it reports to
window logical pixels. `webview::manager::EngineOptions` applies the hardware
acceleration policy, smooth scrolling, and strict-proxy WebRTC on WebKitGTK;
`disable_animations.js` joins the init scripts when
`performance.disable_animations` is on.

## Downloads

`downloads` receives each account's `on_download`. The engine writes the file
to `cache/download-staging/<seq>/`; only on completion do ZapZap's rules
apply: the multiple-downloads question (the session's first one always passes),
then the behavior (`dialog` via the `downloads://prompt` event and the
`download_answer` command, `automatic`, or `ask_every_time` with the native
dialog). The file goes to the configured folder with a sanitized name
(`downloads::naming`) and an ` (N)` suffix when the name exists, enters the
history (`downloads.recent`, 100 entries), and opens on its own if
`downloads::sniff` confirms a PDF or image by content and extension. The
open, reveal, and delete commands only accept history paths.

On Linux, Tauri keeps each account's WebKit context for the app's lifetime,
and wry re-connects its download signals on every recreated webview: the same
completion arrived once per registration, and each registration holds a single
failure flag that never flips back to false, so after one failed or cancelled
download (blocked ones included) every following download of the account was
reported as failed and discarded. Hence, on Linux, `on_download` only decides
the destination (`Requested`); wry's completion is ignored and
`downloads::linux` observes the context once per account, with per-download
failure state, and reports completion via the staging path. Only a download
still being received matches a completion.

Evolution Go chat media (`evo_save_media`, document click, or "Download" in
the message menu) enter the same flow via `downloads::save_native`: the bytes
are staged and follow behavior, folder, history, and auto-open. Because the
download starts from a user click, the multiple-downloads question does not
apply.

## Permissions

`permissions` connects WebKitGTK's `permission-request` signal on each account
webview. Notifications are always granted; camera, microphone, screen,
geolocation, and pointer lock follow `permissions.auto_grant`; the rest asks
the UI via the `permissions://request` event. The request (a GObject) sits in
a `thread_local` table on the GTK thread until `permission_answer`, which
answers via `run_on_main_thread`. A "Yes" stays in `AppState::permissions`
(in memory, per account) and is forgotten when the webview is recreated. Other
request types get the default denial.

## Customizations

`customizations` keeps ZapZap's tree in `data/customizations/` (`global/` and
`accounts/<key>/`, with account 1 at `storage-whats`), each with `css/`, `js/`,
and `previews/css/`. When each page load finishes, `apply_to_webview` swaps
the `<style data-zapzap-custom-css>` tags and runs each JS with `sourceURL`.
Global files apply when the category is active; an account that does not
inherit adds its own after them. `normalize_userstyle` keeps only
whatsapp.com `@-moz-document` blocks (strict on create, edit, and import).
`customizations::files` operates with a name allowlist and path verification;
URL import accepts HTTPS only, 15 s, and 2 MB, with a userstyles.world preview.
The `Customizations.svelte` page writes files immediately and the activation
and inheritance keys only on Save.

## Spell checker

`spellcheck` uses WebKitGTK's Enchant + Hunspell. `lib.rs` sets
`ENCHANT_CONFIG_DIR` to `<data>/dictionaries` before GTK starts, and the app
dictionaries live in `<data>/dictionaries/hunspell/<tag>.{aff,dic}`;
`/usr/share/hunspell` and `/usr/share/myspell` are listed read-only. Tags
follow Enchant normalization (`pt-PT` → `pt_PT`).

- `catalog`: embedded catalog (`src-tauri/resources/hunspell-catalog.json`,
  generated by `scripts/hunspell-catalog.mjs` from a pinned
  `wooorm/dictionaries` commit). Each file is verified by size and git-blob
  SHA-1 before being written via `.part` + rename.
- Selection: up to 10 languages, mapped to installed tags (exact match, then
  language); empty falls back to the system locale dictionary and then to the
  first installed one. Recents: up to 4, newest first.
- `provision` installs the system locale dictionary once
  (`spellcheck.provisioned_locale`); failures are retried at the next start.
- `apply_all` applies languages and state to each account's `WebContext`; runs
  at webview creation, on `spellcheck` changes, and after install/remove.
- The context menu (`linux.rs`) removes navigation and inspect items and adds
  "Check spelling" (stateful `GAction`) and "Languages…", which emits the
  `spellcheck_languages` UI action.
- UI: Language page and Download section, `LanguagePicker`, and
  `DictionaryManager`; the `spellcheck://changed` event updates both.

## Problem reports

`reporting` follows ZapZap's `core/reporting`; nothing leaves the device without
user review.

- `sanitize`: strips personal paths, URL query/fragment, emails, chat IDs,
  tokens, cookies, UUIDs, and phones; in JSON documents it drops secret-looking
  keys and preserves well-formed IDs, versions, and timestamps.
- `document`: manual report (`manual_problem`), panic (`automatic_crash`), and
  unclean previous session (`unexpected_shutdown`); English Markdown and URL
  `github.com/quantforge-br/fastzap/issues/new?title=…` (title only).
- `store`: `<data>/reports/<id>.json`, at most 20 reports and 30 days; statuses
  `pending`, `pending_review`, `copied`, `opened_on_github`.
- `session`: `.session-active` with `startup`/`running`/`shutdown` phases
  (`crashed` after a panic); removed on `RunEvent::Exit`. A leftover marker
  becomes a report on the next run when `reporting.crash_prompts` is on. The
  `TeeLogger` keeps the session's warnings and errors (16 kB) for the logs
option; the panic hook stores the last panic.
- UI: Report a problem page (`debugging`), `ReportDialog` (form and review;
  the UI copies the Markdown and the backend opens GitHub), `ReportsDialog`,
  and the startup notice (only after onboarding).

## Update check

`update` queries `releases/latest` of `quantforge-br/fastzap` once per process
(5 s timeout, silent failures) and only when `build_info` indicates an official
GitHub Actions build of the official repository packaged as DEB, macOS, or
Windows exe; Flatpak, AppImage, RPM, Snap, and local builds never query.
Drafts and pre-releases are ignored, versions are compared numerically, and the
notes link is only accepted under `github.com/quantforge-br/fastzap/releases/`.
The result stays in `UpdateState` (`update_info`) and the `update://available`
event; the UI shows the ⬆ button in the sidebar, which opens a dialog (a
popover would render under the accounts' webviews), and a line on the About
page.

## Onboarding and helper dialogs

When loading the preferences, `+page.svelte` decides once per run: a false
`onboarding.completed` opens `src/lib/onboarding/Onboarding.svelte` (six steps
of ZapZap's `InitialSetupDialog` over a local draft, `draft.ts`); "Skip" only
marks `completed` and "Finish" writes everything in one `settings_patch`,
forcing "Quit" on close when the tray stays off. FastZap shows no donation
prompt. The `open_chat_by_number` (`src/lib/chat/`, validation mirroring
`webview::deeplink`) and `show_shortcuts` (`src/lib/shortcuts/`) actions open
dialogs. All count as overlay, because native webviews would render above the
HTML.

## Translations

`po/*.po` comes from ZapZap (same domain and languages). `pnpm i18n`
(`scripts/po2json.mjs`) generates versioned
`src/lib/i18n/catalogs/<locale>.json`: `{plural, messages, plurals}`, no `fuzzy`
entries, with "ZapZap" replaced by "FastZap". Rust embeds the catalogs at
build time (`build.rs`) and uses `i18n::tr` in the tray, menus, and
notifications; the UI uses `t`/`tn` from `$lib/i18n`, with `Plural-Forms`
evaluated without `eval` (`plural.ts`). The `app_locale` command resolves
`system` to the OS locale; the layout swaps the catalog live when
`interface_language` changes.

## Tray and badge

`tray` renders ZapZap's SVGs with `resvg` (`default`, `symbolic_light`,
`symbolic_dark`) at 128 px, with the red counter (max `999`). The total sums
only enabled accounts and becomes zero with `notification_counter` off; the same
value feeds `Window::set_badge_count` (Linux/macOS); on Windows, where the badge
does not exist, it becomes a taskbar overlay icon (32 px red disc, `99+` above
99). The menu is rebuilt when the language, mute, or floating panel changes. On
StatusNotifierItem trays (Linux), icon clicks never reach the app; only the
menu.

## Notifications

`bridge.js` replaces `window.Notification` and sends `wa_notify`/
`wa_notification_close`. `notifications::policy` applies `notifications.*`, the
account DND, and silent mode (`sound` off or `audio_muted`). The backend is
chosen as in ZapZap: portal on Flatpak, `org.freedesktop.Notifications` (via
`zbus`, with `ActionInvoked` and `ActivationToken`) on other Linux installs,
and `tauri-plugin-notification` on Windows/macOS, which does not report clicks.
A new notification with the same `tag` closes the previous ones. Clicking shows
the window with the activation token, activates the account, and calls
`notificationClicked` in the page, which opens the conversation. Circular
photos live in `<cache>/notification-images/`.

## Evolution Go accounts

Evolution Go accounts create no remote webview: the content area shows
`src/lib/evolution/ChatView.svelte` (pairing, chat list, and conversation),
which talks to the backend through the `evo_*` commands and the
`evolution://event` event (`EvolutionEvent`). The layout follows WhatsApp Web:
`ChatView` defines the `--wa-*` colors (light and dark via
`prefers-color-scheme`) inherited by `Conversation.svelte`, `Avatar.svelte`,
and `Icon.svelte` (Material Icons paths); bubble grouping, day labels, links,
and sender colors live in `format.ts`. `ChatSummary` carries `preview_status`
(ticks of our last message) and `own` (self-chat); receipts and the end of each
send re-emit `Chats` to update the ticks.

- `evolution::client`: REST adapter. `AdminKey` and `InstanceToken` are
  distinct types; non-2xx responses become `EvoError` before opening
  `{message, data}`, and server messages go through `reporting::sanitize`.
  Sends that time out after leaving become `Ambiguous` and are never retried
automatically.
- `evolution::ws`: one socket per instance (`/ws?token=<admin>&instanceId=`),
  ping every 20 s (65 s with no frame at all, not even pong, drops the socket;
  writes and close have a 10 s deadline), 1 s to 60 s backoff, unbounded frame
  size, double `{queue, payload}` decoding, and `instanceToken` stripping. The
  URL carries the admin key and is never logged. A renewal request (`Notify`)
  closes the socket, waits for the server to end the connection (it
  unregisters the instance just before), and reconnects with no `Down` and no
  backoff.
- `evolution::model`: converts whatsmeow events (messages, history, receipts,
  presence, connection) into `Change`/`Event`, unwrapping
  `ephemeralMessage`/`viewOnce`/`editedMessage`/`associatedChildMessage` and
  stripping thumbnails. Business messages (`templateMessage`,
  `interactiveMessage`, `buttonsMessage`, `listMessage`) become text (title,
  body, and footer), header media, and `buttons` (`MessageButton`: only
  `http(s)` links open; quick replies, list rows, and poll options are labels);
  polls (`pollCreationMessage*`) are `poll`; button and list replies, text;
  `ptvMessage`, video. An album's opening message (`albumMessage`) is not
  stored: the photos arrive as their own messages, and a reply to the album
  quotes it as photo or video. Downloads send the server only the media proto
  (`{"imageMessage": …}`) and only when there is a `directPath`; media that
  WhatsApp serves only via `staticURL` (official-account ads) the server does
  not download.
- `evolution::store`: per-account SQLite at `<data>/evolution/<id>/chats.db`
  (chats, messages keyed by `(chat, id)`, reactions, contacts, meta). Delivery
  status only advances (a late send failure does not demote an already confirmed
  message). Each journal page is applied in order in a single transaction along
  with the cursor. The database stores the origin in `meta` (`bind:server` =
  server + instance, `bind:owner` = paired number); when it changes, chats,
  messages, cursor, and the media cache are discarded. Downloaded media lives in
  `<data>/evolution/<id>/media/`. `chats.archived` (schema 2) mirrors
  `GET /chat/settings`, read on every socket connection and every 5 min.
  Archived chats stay in the "Archived" folder, even without messages, and do
  not enter the unread count. `lid_map` (schema 3) stores learned LID → number
  pairs from `phoneNumberToLidMappings` and the HistorySync and
  `/chat/settings` `pnJID`/`lidJID`/`phoneJid`, from the `Sender`/`SenderAlt`
  and one-to-one `Chat`/`RecipientAlt` of live messages (`Change::Alias`,
  applied before the message), and from our own message echoed under an
  unknown LID while its outbox copy sits in a phone chat (the server sends to
  the LID `IsOnWhatsApp` resolves for the number); chats, senders, receipts, and
  "typing" referencing a known LID are stored under the number, and learning a
  pair merges what sat under the LID into the number's chat, keeping the
  furthest delivery status of a message both held. An on-demand
  HistorySync (`syncType` 6) does not overwrite unreads. When migrating from
  schema 2 with LID chats, the journal cursor is wiped to re-read the journal
  and merge the pairs. `messages.quote_kind` (schema 4) stores the quoted
  message's kind; the migration fills it, with the text, from each reply's
  proto. When reading a page, the quote uses the quoted message when it is
  stored (current text, author, document by name) and otherwise the copy the
  reply brought. `messages.buttons` (schema 5, JSON) stores the buttons; the
  migration re-reads from the proto the messages stored as unsupported and
  their quotes, deletes album openings, and rebuilds the affected chats'
  previews. The schema 7 migration merges each LID chat whose own messages
  have copies in exactly one phone chat into that chat; `messages_by_id`
  indexes those lookups and receipts that name an unknown chat.
- `evolution` (session): `sync` starts/stops sessions according to the enabled
  accounts, serialized by a lifecycle lock; stopping the session cancels
  in-flight work. Each session checks the instance via the admin listing and only
  calls `POST /instance/connect` when the client is stopped, the WebSocket is
  down, or a category is missing; the payload preserves webhook, RabbitMQ, NATS,
  and existing events. After each socket connection, it reads `GET /chat/events`
  from the saved cursor (the private journal covers events missed while the
  socket was down); one reader at a time, 404 turns the journal off,
  503/network/timeout retry with 30 s to 30 min backoff, and a backlog larger
  than 200 pages continues right after. With the socket up, the journal is also
  read every 30 s (skipped if another read is in flight; failures wait for the
  next cycle): the server keeps one socket per instance and, when the delayed
  cleanup of an old socket removes the new one's registration, the socket stays
  open (pongs arrive) without receiving events. If the read stores messages the
  socket did not deliver and no event arrived since the previous read, the
  session asks for a new socket. `LoggedIn` only holds while the client is
  authenticated. Without a socket (`Connected` false), an instance with a paired
  JID shows as offline, not as pairing: the server keeps, without retrying, the
  client that did not reach WhatsApp at startup, and generates no QR for it
  (`logged_out` persists until "Link again"). "Try again" restarts that client
  with `POST /instance/forcereconnect/{id}` (admin key, body with the paired
  number), because `POST /instance/reconnect` refuses a disconnected client; the
  500 the server returns when the new client has not logged in 2 s later is not
  an error, and the next refresh decides the state. Unreads go into the
  `UnreadRegistry` (chats with unreads, like the WhatsApp Web title) and new
  messages raise notifications through the common pipeline with `tag` = the
  chat's JID; clicking emits `open_chat`.
- Secrets: the admin key and the instance token live only in the keyring
  (`evolution-admin-key:<id>`, `evolution-instance-token:<id>`). The admin key
  enters only via `evo_instances`/`evo_setup`; no command returns it.
- `GET /instance/qr` restarts a logged-in client to generate a fresh QR, so QR
  and phone code are only requested with `LoggedIn` false.
- Drag-and-drop attachments: `Conversation.svelte` listens for `onDragDropEvent`
  on the `ui` webview, converts the physical coordinates, and accepts the drop
  only over the visible conversation, online and outside editing/sending. Paths
  sit in a removable local queue until confirmed with Send; switching
  conversations discards the queue. `evo_send_file` takes an optional `path`
  (null opens the existing picker), validates regular file and 100 MB limit with
  bounded reads in Rust, and reuses media cache, outbox, and sending. Files go
  out in sequence, with the same caption and reply; a failure stops the batch and
  does not requeue the attempted file, since the server may have received it.
  Leaving the conversation stops the next sends, not the upload already started.

## Startup and shutdown

1. `lib::run` parses the CLI (`app::cli`, in `parse_known_args` style:
   `--setSettings KEY VALUE` with ZapZap's `group/key` keys, `--wayland`,
   `--hideStart`; all other arguments are links). Before GTK, it sets
   `GDK_BACKEND` from `--wayland`, an already exported `GDK_BACKEND`, or
   `system.display_backend` (read without repairing the file).
2. `tauri-plugin-single-instance` delivers a second launch's arguments to
   `app::lifecycle::on_second_instance`: it applies `--setSettings`, restarts
   with `fastzap://restart`, opens the first valid `whatsapp:`/wa.me link, and
   shows the window.
3. `setup` loads preferences, runs the ZapZap migration once, applies
   `--setSettings`, creates window, child webviews, and tray, and installs the
   `SIGTERM`/`SIGINT` bridge (`signal-hook` → `AppHandle::exit`). On Flatpak,
   `app::dbus` claims the `io.github.quantmind_br.fastzap` name and exports
   `org.freedesktop.Application` (`Activate`, `Open`, `ActivateAction`) at
   `/io/github/quantmind_br/fastzap`, forwarding the activation token
   (`activation-token` or `desktop-startup-id`) to the window.
4. The window starts hidden with `start_background` or `--hideStart`, except
   while initial setup is unfinished. First-launch links wait for the page to
   load (`eval_when_ready`).
5. `tauri-plugin-window-state` restores the `main` window's size, position, and
   maximization (never visibility) and saves on exit.
6. Closing the window calls `app::lifecycle::request_close`: with
   `confirm_on_close` the UI asks (with "Don't ask again", also for tray Quit
   and `Ctrl+Q`); then the window goes to the tray, or the app quits if
   `quit_on_close` is on or the request was Quit.
7. `system.start_system` is applied at startup and on every change
   (`autostart`): on Flatpak, the `Background` portal (`RequestBackground` with
   `fastzap --hideStart`); on other Linux, the
   `$XDG_CONFIG_HOME/autostart/io.github.quantmind_br.fastzap.desktop` entry
   pointing at the AppImage or the current executable; on Windows and macOS,
   `tauri-plugin-autostart` (Run key and LaunchAgent).
8. On `RunEvent::Exit` the shown notifications are removed and the D-Bus name
   is released.

## Packaging

- Identifiers: the App ID `io.github.quantmind_br.fastzap` names the Flatpak,
  `.desktop`, AppStream, D-Bus name (`app::dbus` and
  `<APP_ID>.SingleInstance`), keyring service, and the configuration, data,
  and cache folders (`AppPaths`). Tauri's `identifier` is
  `io.github.quantmind-br.fastzap`, because the bundler rejects `_`; it only
  appears in the macOS bundle ID, the Windows AUMID, and the
  `tauri-plugin-window-state` folder. `enableGtkAppId` stays off, so the
  Wayland/WM_CLASS app_id is `fastzap` (`StartupWMClass=fastzap`).
- `pnpm tauri build` produces deb (`fast-zap`), rpm, and AppImage on Linux, NSIS
  and MSI on Windows (arm64 NSIS only), and universal dmg on macOS, with ad-hoc
  signing and no notarization. The deb/rpm `.desktop` comes from
  `packaging/linux/fastzap.desktop.hbs` (`FastZap.desktop`, `fastzap` icon,
  `whatsapp:` and `fastzap:` schemes).
- `make install` uses `packaging/arch/install.sh` and `packaging/arch/PKGBUILD`
  to compile the local tree, convert the DEB into `dsr-fastzap-git`, and
  install via pacman. Only the install transaction uses sudo; the package
  normalizes the launcher to `fastzap.desktop` and stays separate from the
  Flatpak.
- `packaging/share/`: `.desktop`, AppStream metainfo, and SVG icon named after
  the App ID, used by Flatpak and snap; the `src-tauri/icons/` icons are
  generated from that SVG with `pnpm tauri icon`.
- `packaging/flatpak/`: manifest over `org.gnome.Platform` 50 (WebKitGTK 4.1),
  `rust-stable` and `node24` extensions, and `libayatana-appindicator` from the
  `shared-modules` submodule (the tray loads the library at runtime). The app
  module downloads crates and npm packages (`--share=network`), so it suits
  local and CI builds, not Flathub. On Flatpak with the NVIDIA driver,
  `platform::disable_dmabuf_renderer` sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`
  before GTK; without it Wayland drops the connection (Error 71) and the app
  closes.
- `packaging/snap/`: `snapcraft.yaml` (core24, `gnome` extension) copied to
  `snap/` by `prepare.sh`; the D-Bus slot covers the single-instance name.
- `packaging/common/`: `build-info.sh` stamps official builds (`FASTZAP_BUILD_*`,
  read at compile time by `build_info`) and `version.sh` prints and checks the
  version across the three manifests.
- Workflows: `build-{deb,rpm,appimage,flatpak,snap,windows,macos}.yml` (manual
  or called), `release-candidate.yml` (`X.Y.Z-rc.N` tag and pre-release), and
  `release-deploy.yml` (published release). Shared setup lives in
  `.github/actions/setup-build/`. The `build` job runs with `contents: read`
  and no persisted credential; only the `publish` job has write access and
  attaches artifacts with `release-upload.sh`, which requires an existing
  release and a tag matching the version (optional `v` and `-rc.N`). Third-party
  Actions stay pinned by SHA.
