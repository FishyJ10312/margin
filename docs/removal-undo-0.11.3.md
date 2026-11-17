# Undo and redo catalog removal

After a confirmed catalog removal, use the footer's Undo removal action or
Command+Z (Ctrl+Z on Windows). It restores the photos' previous order, active
preview, selection and Shift anchor. Consecutive removals can be undone in
reverse order, including Clear list and removal of the last photo. Originals
are never deleted or rewritten.

The local catalog retains up to 20 removal operations across app restarts. The native core
holds metadata with no retained thumbnail buffers; existing small thumbnails
stay in the local disk cache. Evicted records release unused cached thumbnails.
Undo restores records without inspecting or decoding original photo files.
The ordinary preview pipeline handles the selected source afterwards.

Restoration changes catalog membership and order only. Current frame/watermark
settings, presets and export jobs are preserved. New imports stay in the catalog.
If a removed path was reimported, restoration uses its current record rather than
creating a duplicate and maps the saved selection to that record's ID.

Removal and restore commit the catalog atomically before consuming history.
Save failures leave the current catalog and undo record intact for retry. Receipt
IDs enforce last-in-first-out ordering. Import/start/export activity prevents
undo; repeated shortcuts during one restore cannot dispatch it again. Text,
number, font search, content-editable controls, IME, menus and open dialogs keep
their own keyboard behavior, including native text undo and redo.

Checks: `npm run check`, `node --experimental-strip-types --test
tests/photo-selection.test.ts`, `cargo test --lib removal_undo_tests`, and
`ego-browser nodejs < tests/ui/removal-undo-acceptance.mjs` with Vite running.
Native Mac acceptance uses a separate synthetic-photo profile before installation.

Command+Shift+Z (Ctrl+Shift+Z on Windows) or the footer's Redo removal action
replays the most recently undone removal, including Clear list. Consecutive
undo and redo follow operation order. Redo always targets that operation's photos,
independently of the current selection; undoing that redo restores the selection
from just before redo. Settings and export state remain current.

A successful new removal or an import that actually adds photos clears the redo
branch. Cancelled confirmations, failed saves, duplicate/unsupported imports,
selection changes, and setting edits preserve it. Both stacks share the existing
20-operation persistent limit. Metadata and IDs only are retained, without additional
image buffers, dependencies or network calls. A failed redo keeps both the catalog
and receipt available for retry. `tests/ui/removal-redo-acceptance.mjs` covers the
full keyboard and button flow with an isolated mocked native bridge.

## Native Mac verification

On 2026-11-17, six synthetic PNGs were tested in an isolated native app. Removing photos 2–5 and then photo 6 changed the catalog 6 → 2 → 1. Consecutive Command+Z restored 1 → 2 → 6, including the original order, four-photo selection, active photo 5 and Shift anchor at photo 2. All six were then removed; the empty-screen Undo removal button restored all six selected. A final single removal verified native text undo separately: Command+Z in the HEX field reverted the edited color without restoring the photo, and Command+Z on the photo list still restored it afterwards. The six source hashes remained unchanged. The installed user app was backed up and updated, with its existing catalog, settings, export queue, model configuration and authorization files preserved.


## Native Mac redo verification

On 2026-11-17, the standalone redo candidate used six synthetic PNGs and an
isolated app profile. Native Command+Shift+Z replayed the four-photo removal,
then a later single-photo removal. Two undos restored all six in their original
order; repeating redo and undo preserved operation order and selection. Native
text undo/redo in the HEX field did not consume photo history; focusing a photo
then replayed the pending removal successfully. Removing all six, undoing via the
footer, redoing via the footer from the populated view, and undoing from the
empty view restored all six selected. All six source hashes were unchanged.
The installed Mac bundle was backed up, replaced, signature-checked and opened;
the real catalog, settings, presets, export queue, model configuration and saved
authorization files matched the pre-update backup. No real model calls ran.


## Persistent history

The catalog journal atomically contains membership, order, the current selection,
and both history stacks. Entries retain the selection and Shift anchor from
before their removal. Startup returns compact history receipts so the same
Command+Z / Command+Shift+Z actions work after a restart, including an empty list.
Both stacks share a total limit of twenty operations. No original photo files
are copied or modified. History metadata has no retained thumbnail buffers.

Ordinary selection changes write only a small atomic local sidecar. A catalog
revision rejects delayed writes from before an import, removal, undo or redo.
The UI serializes and coalesces these writes; membership operations always
commit their own selection with the journal. Invalid history is reset without
resetting the current catalog. Legacy state loads with empty history; previous
versions did not save their session-only history to disk. Journal backup recovery
restores history and catalog together.

Validation: the Rust library suite passed 135 checks (one optional OS credential
check remains ignored), six selection tests passed, and the persistent-history
UI fixture passed repeated reload, selection, branching, failed-save retry and
stale selection-write checks. Native Mac process restart checks are recorded
with the packaged candidate's acceptance evidence.


Native persistence acceptance on 2026-11-17 used six synthetic PNGs and five
actual process restarts of the standalone Mac candidate. It verified undo, redo,
both stacks together, order, current preview, multi-selection, the Shift anchor,
changed pre-redo selection, and clear-all recovery from an empty catalog. All
six source hashes were unchanged. The signed installed app was updated and
opened with the existing user catalog, settings and model sign-in files intact.
