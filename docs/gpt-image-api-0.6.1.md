# 留白 0.6.1：直接选择 GPT 图像模型

“生成手绘素材”现在提供两个独立入口：本机 Codex 复用现有登录；GPT Image API 使用用户自己的 API Key，直接选择图像模型。Key 不作为 Codex 登录使用，也不会写入设置、模板或素材来源记录。

## 使用

1. 导入照片，点击“生成手绘素材”。
2. 生成服务选择“GPT Image API · 自选模型”。
3. 默认 Base URL 为 `https://api.openai.com/v1`，在软件内输入 OpenAI API Key。兼容服务需支持 GPT Image 的 Images API、JSON 参考图输入及 Base64 透明 PNG 输出。
4. 从下拉框选择模型，也可点“读取可用模型”查询 `/models`，或手动填写模型 ID。接口返回的模型列表不代表该项目一定拥有生图权限；仍由服务端权限控制。未列出的当前选项会明确标注。
5. 选择低/中/高质量和素材画布大小。默认中质量、1024×1024，每次只生成一张。
6. 输入风格方向，按需勾选照片参考，确认目标接口、模型和额度来源后点击生成。结果先显示本机装框预览，再应用或保存模板。

默认不发送照片。开启参考时仅传入已展示、最长边 768 px、无原始元数据的 JPEG 缩略图，并要求模型只借鉴配色与气氛。照片和水印仍由本机合成。生成素材保存后，切换照片、缩放、批量导出和模板复用不再次调用模型。

## 模型选择

内置官方文档列出的 GPT Image 2.5 Flare / Sunburst、2.6 Flare / Sunburst、GPT Image 2、1.5、1 mini 及 `chatgpt-image-latest`。默认 `gpt-image-2.5-flare`；Flare 适合快速生成，Sunburst 偏向精细编辑。实际可用模型由账号、服务商和权限决定。模型 ID 可手动填写以支持后续更新或兼容服务命名。

已经停用的 `gpt-image-1` 不出现在列表中，也不会作为旧配置被默默调用。选择模型、质量或尺寸不会自动生成；任何关键选项改变都需要重新勾选同意。

## 请求路径与稳定性

- 无参考：`POST /images/generations`。
- 有参考：`POST /images/edits`，JSON `images: [{image_url: dataURL}]`。
- 共同参数：明确模型、`n: 1`、`background: transparent`、`output_format: png`、所选质量与尺寸。GPT Image 不接收旧式 `response_format` 参数。
- 只接收 Base64 PNG，不自动跟随模型返回的下载链接。沿用 PNG 大小、尺寸、透明像素与可见内容检查，素材独立保存。
- 图片响应上限 30 MiB，PNG 文件上限 20 MiB，最长边上限 4096；解码和归档在 Rust 后台执行，缓存仍为最多两份 2048 px RGBA 图。
- 只读模型列表限 2 MiB、20 秒；生成连接有超时、取消和单任务限制。未知时长使用不定进度，不自动重试，不回退至另一家付费服务。
- HTTP 只接受本机 localhost，其他地址需要 HTTPS，禁止 URL 内嵌用户名、密码、查询参数和片段，不跟随重定向转发 Key。
- Key 仅保留在本次应用会话的内存。切换接口主机自动清除已有 Key，防止旧 Key 跟随新地址；关闭软件后需要重填。服务端错误正文不直接显示，避免回显凭据。
- 401、403、404、429、参数不支持等错误分别说明。取消本机等待不保证服务端停止计算或免计费。

## 额度与验证边界

直连模式消耗所填 API 项目的额度；不使用本机 Codex CLI 的登录或额度。生成质量和尺寸会影响费用与耗时，软件不硬编码可能过时的报价。

本次用本地模拟 Images API 验证模型选择、请求、透明素材归档、参考图路径、认证错误脱敏、取消与原生 UI 流程。没有使用用户的真实 API Key 或消耗真实付费图片 API 额度；云端账号权限与实际生成仍需用户在软件内填写自己的 Key 后验证。

官方来源（2026-11-15 核对）：[图像生成指南](https://developers.openai.com/api/docs/guides/image-generation)、[生成接口](https://developers.openai.com/api/reference/resources/images/methods/generate)、[编辑接口](https://developers.openai.com/api/reference/resources/images/methods/edit)。
