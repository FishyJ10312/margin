# Margin 0.11.4 — Public preview / 公开预览版

Margin is a lightweight, local-first photo framing and watermark app.
本机批量加边框、水印和导出，无需安装开发环境。

## Download / 下载

| Platform / 平台 | Installer / 安装包 |
| --- | --- |
| Mac (Apple Silicon M1–M-series) | `Margin-0.11.4-macos-arm64.dmg` |
| Mac (Intel) | `Margin-0.11.4-macos-x86_64.dmg` |
| Windows x64 (Intel or AMD) | `Margin-0.11.4-windows-x64-setup.exe` |

Mac: Open the DMG, then drag Margin into Applications. macOS 12+ is the configured minimum. Windows: run the setup executable; it installs required components, including WebView2 if needed (an internet connection may be required). Core framing, watermarking, and export do not require Node, Rust, Codex, an API key, or a subscription.

Mac：打开 DMG，将 Margin 拖入 Applications（应用程序）。配置最低系统版本为 macOS 12。Windows：运行 setup.exe，按提示安装；必要时安装程序会下载 WebView2。加框、水印和导出功能不需要 Node、Rust、Codex、API Key 或订阅。

**Signing notice / 签名说明:** This is an early preview. The Mac builds use ad-hoc signatures and have not been Apple-notarized; the Windows installer is not Authenticode-signed. macOS Gatekeeper or Windows SmartScreen may show a warning. Verify the file against `SHA256SUMS.txt` and install only if you trust the publisher. Do not disable system protections globally. On Mac, see [Apple's guidance for app warnings](https://support.apple.com/en-us/102445).

本版尚未经过 Apple notarization 公证，Windows 安装包也未做 Authenticode 签名。系统可能提示未知开发者。请核对 SHA256、确认信任来源后再安装；无需关闭系统安全防护。

## What's new / 更新内容

- Gallery interface with dark/light themes, English and Chinese, realistic film frame templates, precise portrait/landscape adaptation.
- AI image generation and one-step refinement with elapsed progress and local preview recovery; each new model request requires consent.
- Improved multi-selection, template application, independent/shared settings, undo/redo, and transactional MCP edits.
- Owned PNG/font assets, aligned generated frame openings, more readable result cards, and reliable preview/export parity.
- Local JPG/PNG/HEIC imports, batch export without overwriting originals, and optional Agent/MCP control.

AI features are optional. Connect your own supported model account or API, or use your separately installed and logged-in Codex for image generation. Only the explicitly approved photo thumbnail is sent when an AI request is made. Native HEIC decoder libraries, bundled fonts and required notices are provided inside both installers.

AI 是可选功能；普通加框完全离线。使用艺术边框时，可按模型配置连接自己的账户或 API；本机 Codex 生图需要用户另外安装并登录 Codex。模型额度由用户自己的服务提供。

## Verification / 验证

Release assembly requires successful native tests and installer smoke tests on GitHub-hosted macOS arm64/Intel and Windows x64 runners, including real JPG, PNG and HEIC import, preview, Chinese watermark, export and unchanged source files. This is not proof of all OS versions or of signing/notarization on clean user devices. Live AI generation/refinement was separately exercised on the local Mac; installer tests do not spend model credits or include private photos.
