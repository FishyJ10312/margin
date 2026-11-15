# 第三方组件

- Tauri：MIT／Apache-2.0，https://github.com/tauri-apps/tauri
- Svelte：MIT，https://github.com/sveltejs/svelte
- Lucide：ISC，https://github.com/lucide-icons/lucide
- image：MIT／Apache-2.0，https://github.com/image-rs/image
- COSMIC Text：MIT／Apache-2.0，https://github.com/pop-os/cosmic-text
- lcms2 Rust wrapper：MIT，https://github.com/kornelski/rust-lcms2
- Little CMS：MIT，https://github.com/mm2/Little-CMS
- libheif 1.23.7：LGPL-3.0，https://github.com/strukturag/libheif/releases/tag/v1.23.7
- libde265 1.1.3：LGPL-3.0，https://github.com/strukturag/libde265/releases/tag/v1.1.3
- Noto Sans SC Regular：SIL Open Font License 1.1，完整许可证位于 `src-tauri/resources/FONT-LICENSE.txt`，https://github.com/notofonts/noto-cjk
- ZCOOL XiaoWei（站酷小薇）：SIL Open Font License 1.1，`licenses/zcoolxiaowei-OFL.txt`，https://github.com/google/fonts/tree/main/ofl/zcoolxiaowei
- Ma Shan Zheng（马善政）：SIL Open Font License 1.1，`licenses/mashanzheng-OFL.txt`，https://github.com/google/fonts/tree/main/ofl/mashanzheng

三种完整中文字体随应用分发，不安装到系统。新增两种字体的下载来源与 SHA256 见 `src-tauri/resources/fonts-sources.json`。

原生解码库使用可替换的动态链接方式。构建脚本记录了源码版本、编译方式和解码选项；随 Mac 交付包另附 LGPL 许可证与对应原生库源码归档。

源代码依赖的精确版本以 `package-lock.json` 和 `src-tauri/Cargo.lock` 为准。

可选订阅连接使用用户另外安装的 Pi AI（`@earendil-works/pi-ai`，MIT）以及 Pi 的凭据存储模块，参考 https://github.com/earendil-works/pi/blob/main/packages/ai/README.md 。本机验证版本 1.5.2；SDK、Pi coding agent 和 Node 不包含在应用包或源码交付包中。适配器只加载 AI、指定服务商与凭据模块，不启动 coding agent、扩展、MCP 或工具。Pi 连接与本机 Pi 共用该服务商的授权，原生 ChatGPT 入口则继续使用本应用自己的系统凭据库。

- reqwest：MIT／Apache-2.0，https://github.com/seanmonstar/reqwest
- Tokio：MIT，https://github.com/tokio-rs/tokio
- rustls：Apache-2.0／ISC／MIT，https://github.com/rustls/rustls
- ring：许可证见附带通知，https://github.com/briansmith/ring
- jsonwebtoken 9.3.1：MIT，验证 ChatGPT OIDC 身份签名；`licenses/jsonwebtoken-LICENSE.txt`，https://github.com/Keats/jsonwebtoken
- keyring 3.6.3：MIT／Apache-2.0，通过 macOS Keychain / Windows Credential Store 保存本应用的 OAuth 授权；许可证位于 `licenses/keyring-LICENSE-*.txt`，https://github.com/open-source-cooperative/keyring-rs

六种边框纹样由本项目程序绘制。Hero Patterns 和 ambientCG 作为资料来源列于使用说明，没有将其第三方图片或 SVG 打包进纹样库。

艺术场景的曲线与图案由 tiny-skia / tiny-skia-path 0.12（BSD-3-Clause）在本机栅格化，许可证随包附带。图案路径为项目内置数据，不加载外部 SVG 或脚本。https://docs.rs/tiny-skia/0.12.0/

Rust 图片缓冲使用 mimalloc 0.1.52（MIT，底层 libmimalloc-sys 0.1.49）；用于减少连续大图处理时的空闲大块内存驻留。完整许可证随包附带。https://github.com/microsoft/mimalloc
