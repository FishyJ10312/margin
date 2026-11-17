# Photo catalog removal, 2026-11-17

This revision replaces the small preview-header X with a labeled, 32 px-high
Remove photo button. The button, Backspace, Delete, photo context menu and
Clear list share one native HTML confirmation dialog. It shows the target name
and thumbnail (or the total count), and states that original files stay on disk.
Cancel is initially focused. No new filesystem deletion command was added.

The shortcut ignores text/number/search/select fields, content-editable regions,
font listboxes, IME composition, held-key repeats, modifier combinations and
other open dialogs. Import/start/export activity also prevents removal. A failed
catalog save leaves the dialog open and the photo in the list. Repeated confirm
input cannot dispatch the operation twice. After removal, selection moves to the
next available photo; deleting the last photo shows the existing import screen.

The context menu selects the clicked photo, supports Shift+F10, arrow keys,
Enter, Escape and Tab, and stays inside viewport bounds. There is one reused
menu element, and confirmation uses the already cached small thumbnail.
No dependency, runtime, original-image decode, model request or permission was added.

## Mac verification of the final r4 app

The independently identified Margin Photo Actions app was tested with two small
synthetic PNGs, keeping the installed Margin and its user workspace separate.

- Right-clicking the unselected second image opened Photo actions; its command
  opened confirmation for **photo-2.png**. Confirming changed the catalog from
  two photos to one and selected/focused photo-1.png.
- Backspace on the remaining photo opened its confirmation; Escape kept it.
- Shift+F10, Down and Enter opened the same confirmation through the menu.
- The larger header button opened the same confirmation.
- Backspace in the watermark field changed SignatureX to Signature, without
  opening removal or changing the catalog. Watermark was turned off afterwards.
- Both original synthetic PNG hashes were checked after the catalog removal.
  The installed user app executable is unchanged. Its existing workspace stays
  separate from the synthetic-photo test profile. The existing catalog had
  already changed since the earlier privacy review, before these feature tests;
  no attempt was made to overwrite that user state with the older fingerprint.
- Svelte checks: zero errors, zero warnings. The final app was built with the
  existing path remapping and checked by the Mac packaging guard.

Earlier mocked-storage UI checks cover failure/retry, duplicate confirmation,
clear-all, the last-photo state, Chinese/English copy and export guards. Those
checks initially used a mocked native menu. The confirmation dialog also fit
900 x 650 without horizontal overflow. The existing preview-revision regression
passed. These are scoped evidence, not new Windows/minimum-OS acceptance.

## Context-menu implementation choice

Mac automation could display the first native menu but did not complete its
callback reliably. A subsequent WebKit auto-popover was observed opening at
valid bounds and then closing automatically during the context gesture.
The final menu therefore uses a small, explicitly controlled fixed panel,
with outside-click/keyboard dismissal and viewport clamping. It avoids those
native-menu/popover lifecycle paths. The modal confirmation still uses
`dialog.showModal()` and browser focus containment. Temporary diagnostic
messages were removed before the final build. Earlier test packages are retained
as local evidence and are not the final trial package.

The exact final package, source archive, checksums and current native results are
recorded under the separately named photo-actions-r4-20261117 local artifacts.
Version remains 0.11.3. No public publication, paid operation, system permission
change or real model call was performed. Existing release-rights, signing and
other-platform gates retain their earlier scope.
