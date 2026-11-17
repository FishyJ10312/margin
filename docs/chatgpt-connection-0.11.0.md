# Margin 0.11.0 — Built-in ChatGPT connection

The default model configuration has one ChatGPT entry. Connect ChatGPT opens the official OpenAI authorization page, then returns to the app through a temporary IPv4 loopback listener. Node, Pi, Codex and an API key are not required for this connection. Optional Pi providers remain under Other connections; the duplicate Pi/OpenAI entry is removed from the UI and old Pi/OpenAI photo-analysis selections resolve to the built-in connection. Pi tokens are not copied or rotated by this migration. Image-generation settings remain separate.

## Authorization and storage

The Rust implementation uses PKCE, state, nonce and signed ID-token validation. It retains each account's issued client ID, verified identity and the same device host ID. New registrations identify the app as Margin. Identity-only login does not permit model requests. Models are read from the selected account at the official API endpoint; saving configuration never starts an inference or uploads a photo. Subscription inference continues to require `store:false` and `stream:true`, with bounded image inputs and terminal-event validation. There is no API-billing fallback.

On macOS, new authorization records use an app-owned `chatgpt/credentials` directory under the existing application data directory. Records are JSON containing sensitive OAuth tokens, protected by the current macOS user's file access permissions: directory 0700, records 0600. This is file-permission protection, not separate Keychain encryption. They are bounded in size, written atomically and synchronized to disk; symlinked credential files are rejected on reads. Refresh commits access token, expiry, scopes and rotating refresh token together. Credentials are never returned to the webview, included in ordinary settings, presets, templates, diagnostic output, source archives or logs. Disconnect removes only the selected new authorization record.

Old Keychain entries are left untouched. The native Mac path no longer reads them or triggers their access-control dialogs. A user who only has an old Keychain authorization reconnects through the browser once; the existing account identity and registration are retained. Windows continues to use its native credential store; this release does not claim a new Windows build or hardware test.

Storage follows [OpenAI's native-app credential guidance](https://developers.openai.com/siwc/token-sharing-open-source/sign-in#5-store-credentials-in-a-local-file). The old failure was reproduced as an authorization read failure while macOS reported the default Keychain unlocked, readable and writable. A failed read cannot establish the exact old item's ACL or signing cause, so the change does not claim to diagnose or repair the user's entire Keychain.

## Connection states

A saved account is not displayed as a verified connection. The UI verifies the model catalog whenever the account is selected or reopened. Only a successful nonempty catalog enables the verified indicator and Save as analysis model. Verification clears immediately on retry, account change, or failure. Missing/unreadable authorization requires reconnect; a temporary network failure preserves authorization and offers Retry connection. Identity-only authorization offers Enable ChatGPT plan usage. Cancel and closing the dialog cancel the pending browser sign-in. Account management and optional connections start collapsed.

The backend separately reports saved, connected, reconnect, unavailable and signedOut states. No account token or password is needed to display these states. Normal photo framing, watermark preview, local templates and export do not depend on model connectivity.

## Verification

Regression tests reproduce the formerly contradictory connected state after credential loss. Local OAuth fixtures exercise callback validation, account isolation, model verification, refresh rotation, cancellation and a real file-store relaunch. Browser fixtures test primary login without any Pi runtime calls, lost authorization, model-list outages/retry, pending verification, cancel, account switching, English/Chinese and a 900×650 window. These checks do not spend model credits or constitute a live provider login. Native Mac acceptance is recorded separately with the delivered build.
