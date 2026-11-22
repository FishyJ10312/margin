# Editing, preview and export consistency

- Explicit changes apply across the selected photos even when the active photo already has the requested value. Unmentioned settings remain independent; a template with **Keep composition** enabled preserves each photo's ratio and respects the minimum film padding.
- Shared editing applies only to photos that inherit shared settings. A photo with its own settings continues to preview and export that composition. **Use shared settings** removes the independent composition explicitly.
- The settings undo record includes each affected photo's independent settings. Catalog removal, undo and redo wait for pending settings writes and retain the photos' original files, order and selection.
- Watermark PNGs and custom fonts (including camera watermarks) are copied into the application's managed asset storage. A successful settings commit returns the managed paths so a moved or changed external source cannot unexpectedly change the render.
- The Agent settings and template commands commit before changing the visible settings. Failed Agent edits do not become queued changes. The workspace response also includes effective preview settings separately from the editable inspector settings.
- A model result is retained when its local preview fails or is canceled. Retrying local rendering does not invoke the model again; each new model request needs fresh consent.

The UI/MCP integration tests live in `tests/ui/settings-consistency-acceptance.mjs` and `tests/ui/image-generation-recovery-acceptance.mjs`. The Rust tests cover owned asset storage, export consistency, failure isolation and cancellation/restart.
