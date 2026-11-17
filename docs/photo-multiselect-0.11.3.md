# Catalog multi-selection

Click one thumbnail to select it. Command-click (Control-click on Windows) toggles
individual photos. Shift-click selects the contiguous catalog range from the
last ordinary or modifier-click anchor; repeated Shift-clicks can shrink or
reverse that range. Command/Control + Shift adds a range to the selection.
The current preview has a second inner outline, while all selected thumbnails
have a green border. The footer and removal button show the selection count.

Command+A (Ctrl+A on Windows) selects the full catalog without changing the
current preview. Escape clears selection while keeping that preview visible.
Text, search, select, content-editable and font-list controls keep their normal
keyboard behavior; open menus and dialogs handle Escape first. The selected
count stays visible at zero and on compact windows. With zero selected, the main
export action and photo removal are disabled. Export options retains an explicit
All photos range. Export and removal otherwise use the same current selection.

To select a continuous range, first click one photo normally, then hold Shift
and click another: both endpoints and every photo between them are selected.
Repeated Shift-clicks keep the original anchor, in either direction.

Backspace, Delete and the header removal button confirm removal of the selection.
Right-clicking a selected thumbnail preserves the group. Right-clicking an
unselected thumbnail selects only that photo. The menu, including Shift+F10,
opens the same confirmation. Multiple targets show the selected count and state
that original files remain on disk. Cancel preserves the complete selection.
No selection means no removal. A failed catalog save preserves the photos and
selection, with retry available in the dialog; duplicate submission is blocked.

Selection is small, in-memory catalog state. It does not alter stored image
settings, photo sources or account files. Export defaults to the selection and
also offers the complete catalog, as described in [selected export](selected-export-0.11.3.md). Rendering still follows
one current preview. The existing native `remove_photos` operation removes only
catalog records after confirmation. Import/start/export and text-editing guards
remain in place. No dependency or model request was added.

Automated validation:

- `node --experimental-strip-types --test tests/photo-selection.test.ts`
- `npm run check`
- With Vite running: `ego-browser nodejs < tests/ui/photo-multiselect-acceptance.mjs`

The UI fixture covers Command selection/toggling, fixed-anchor Shift ranges,
Command+Shift unions, empty selection, menu membership, cancel, exact batch IDs,
save failure/retry, duplicate submission, focus after removal, editor/modal
guards, re-import after an empty catalog, Chinese/English counts and export
guards. It uses synthetic photos and mocked storage; native Mac batch removal
and original-file integrity are verified separately before installation.

Native Mac verification (2026-11-17): six synthetic PNGs were imported into
an independently identified app profile. Command-click selected photos 1, 3
and 5; Backspace showed a three-photo confirmation and Cancel preserved the
selection. Shift-click selected photos 2–5, then shortened the range to 2–3.
Right-clicking inside a selected group kept all three selected and confirmed
one batch removal (six catalog records to three). Command-selecting two of
the remaining photos and confirming Backspace reduced the catalog to one,
04-sample.png. All six original SHA-256 hashes remained unchanged. Current
user photos and account data were not used in these removal tests.

## Selection shortcuts verified on Mac

The 2026-11-17 keyboard revision passed five pure selection tests and the isolated selection-shortcuts UI fixture. On the Mac test app, Command+A selected all six synthetic photos; Escape cleared to zero and disabled selection export/removal without blanking the preview. An ordinary click on photo 2 followed by Shift-click on photo 5 selected exactly photos 2 through 5. Export wrote exactly those four JPEGs, and confirming removal removed the same range, leaving photos 1 and 6. All six source hashes remained unchanged. Escape canceled a removal dialog while preserving its selection. The installed user app was then backed up and updated, with user photos, settings, queue and model authorization files preserved. No real model calls were made.
