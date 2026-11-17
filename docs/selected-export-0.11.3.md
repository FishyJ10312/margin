# Export selected photos

The main export action uses the current selection and shows its count. The
review dialog defaults to Selected photos and retains an explicit All photos
option with the full catalog count. Its summary and confirmation button update
with the chosen range. If the selection is empty, the main action shows zero
and is disabled. Export options still offers All photos as an explicit choice;
the range does not silently expand after Escape clears the selection.

The range uses native radio inputs and a fieldset legend. The dialog retains
its header and footer on compact screens while its settings body scrolls.
Existing output folder, format, quality and size options remain shared.

On confirmation, photo IDs and render settings are captured before opening an
asynchronous directory chooser. Preparing or dispatching a batch prevents a
second start. Canceling the directory chooser starts no batch; a start failure
stays visible in the dialog for retry. Once started, the existing backend owns
the fixed list of job items, so later preview selection does not change it.
Original images remain in place and exports use the existing atomic copy flow.
No backend command, dependency, account operation or model call was added.

Validation: `npm run check` and the isolated
`tests/ui/selected-export-acceptance.mjs` cover selected/all counts, keyboard
range switching, exact target IDs, directory cancellation, start failure/retry,
double-start protection, selection changes during a batch, Chinese copy,
no-selection behavior and the 900 x 650 layout. The English single-photo button
uses singular wording. Native Mac export-file checks are recorded separately.

## Native Mac verification

On 2026-11-17, four synthetic 640 x 480 PNGs were imported into an isolated Mac profile. Selecting 01-sample.png and 03-sample.png produced exactly two JPEGs. Switching the range to All photos produced all four JPEGs in a separate folder. Each output measured 688 x 528 pixels with 5 percent padding; all four source hashes stayed unchanged. The header continued to show the two-photo selection after the all-photo batch. The English single-photo label and the installed app's Chinese selected/all range controls were observed. Existing user photos, settings, queue and account files were preserved during installation.
