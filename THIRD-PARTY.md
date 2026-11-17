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

原生解码库使用可替换的动态链接方式。构建脚本记录了源码版本、编译方式和解码选项；Mac 应用的 `Contents/Resources/Licenses/NativeSources` 内附对应源码归档、SHA256 与构建说明，许可证位于上级 `Licenses` 目录。完整依赖声明和通知汇总于 `licenses/THIRD-PARTY-NOTICES.txt`；这是保守包含构建依赖的清单，不代表法律审核结论。

源代码依赖的精确版本以 `package-lock.json` 和 `src-tauri/Cargo.lock` 为准。

可选订阅连接使用用户另外安装的 Pi AI（`@earendil-works/pi-ai`，MIT）以及 Pi 的凭据存储模块，参考 https://github.com/earendil-works/pi/blob/main/packages/ai/README.md 。本机验证版本 1.5.2；SDK、Pi coding agent 和 Node 不包含在应用包或源码交付包中。适配器只加载 AI、指定服务商与凭据模块，不启动 coding agent、扩展、MCP 或工具。Pi 连接与本机 Pi 共用该服务商的授权，原生 ChatGPT 入口由应用内置；Mac 使用仅当前用户可读的本机授权文件，Windows 使用系统凭据库。

- reqwest：MIT／Apache-2.0，https://github.com/seanmonstar/reqwest
- Tokio：MIT，https://github.com/tokio-rs/tokio
- rustls：Apache-2.0／ISC／MIT，https://github.com/rustls/rustls
- ring：许可证见附带通知，https://github.com/briansmith/ring
- jsonwebtoken 9.3.1：MIT，验证 ChatGPT OIDC 身份签名；`licenses/jsonwebtoken-LICENSE.txt`，https://github.com/Keats/jsonwebtoken
- keyring 3.6.3：MIT／Apache-2.0，Windows 凭据库及旧版兼容实现；当前 Mac 原生 ChatGPT 登录使用仅当前用户可读的本机授权文件，不读取旧钥匙串。许可证位于 `licenses/keyring-LICENSE-*.txt`，https://github.com/open-source-cooperative/keyring-rs

六种边框纹样由本项目程序绘制。Hero Patterns 和 ambientCG 作为资料来源列于使用说明，没有将其第三方图片或 SVG 打包进纹样库。

艺术场景的曲线与图案由 tiny-skia / tiny-skia-path 0.12（BSD-3-Clause）在本机栅格化，许可证随包附带。图案路径为项目内置数据，不加载外部 SVG 或脚本。https://docs.rs/tiny-skia/0.12.0/

Rust 图片缓冲使用 mimalloc 0.1.52（MIT，底层 libmimalloc-sys 0.1.49）；用于减少连续大图处理时的空闲大块内存驻留。完整许可证随包附带。https://github.com/microsoft/mimalloc


模板预览使用本机生成的纯白样图，不包含第三方样照。

`tests/example.heic` 仅为离线测试使用的 libheif 上游示例，与上述
libheif 1.23.7 对应源码归档中的 `examples/example.heic` 字节一致，不随
Mac 可执行程序加载。SHA256 为
`7f8b363e4936c0666a25f64f3a92fda10bd8e5453be4592530b65a55dd98f3f2`。
其余 HEIC 测试样本由本机合成；生成的验收图片不放入源码交付包。

`src-tauri/test-fixtures/oauth-test-only.pem` 是公开、固定的本地模拟 OAuth
测试签名密钥，与测试 JWKS 配对，仅由 `#[cfg(test)]` 模块引用；不代表真实
账户凭据，也不进入应用可执行文件。其固定散列由源码资产清单校验。

前端实际渲染的依赖还包含 clsx 2.1.1 与 esm-env 1.2.2（MIT），完整通知已
纳入附带清单。可选 Pi SDK 和用户本机 Codex 的版本、请求与计费行为由各自
软件决定，不属于本包内置组件的覆盖范围；内置适配器对 Pi 显式设置零重试。
