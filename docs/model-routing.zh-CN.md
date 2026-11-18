# Margin 的分析配框、生图和胶片模板

打开 **艺术边框 / Art frame**，首先选好 **生成方式 / Generation method**。下面显示的服务和模型就是这一轮会调用的入口。

| 我想做什么 | 在 Margin 里怎么配置 |
| --- | --- |
| 用现有 ChatGPT 订阅生成可编辑的配框 | **模型配置 → ChatGPT → 连接 ChatGPT**。完成网页登录和用量授权，选账号列出的模型，点 **保存为照片分析模型**。回到艺术边框，选 **分析配框 · 可编辑**。这个入口无需 API Key。 |
| 画出完整插画边框或独立素材，使用现有 Codex 登录 | 先在本机 Codex 登录。在 Margin 打开 **模型配置 → 图像生成 → 本机 Codex**，点 **保存图像生成设置**。回到艺术边框，选 **完整插画边框 · 生图**。Codex 的高级推理模型可以留空。 |
| 自己指定 GPT Image 模型 | **模型配置 → 图像生成 → GPT Image API**。选择模型并填写该服务的 API Key，点 **保存图像生成设置**。使用 OpenAI 官方 Images API 时，Key 属于 API 项目。Margin 只在本次运行中保留 Key。 |
| 只想套一个胶片框 | **边框 → 全部模板 → 胶片**。选样式、看预览，再点应用。胶片模板在本机绘制。 |

ChatGPT 账号连接和图像 API 是独立设置。如果生成页显示 **GPT Image API**，这一轮需要图像 API 的授权，即使分析模型已经通过 ChatGPT 登录。图像 API 的账单由 API 项目管理。[OpenAI 的 ChatGPT 与 API 计费说明](https://help.openai.com/en/articles/9039756-managing-billing-for-chatgpt-and-the-api-platform)

## GPT‑6.2 Sol 在两种入口里各做什么

- 通过 **ChatGPT → 分析配框**选 GPT‑6.2 Sol 时，它分析照片缩略图并返回可编辑的设计参数。边框由 Margin 根据参数绘制。若通过 **兼容视觉 API**填同一模型 ID，效果仍属于分析配框，授权和用量走指定的 API 项目。 [GPT‑6.2 Sol 模型说明](https://developers.openai.com/api/docs/models/gpt-6.2-sol)
- 在 **本机 Codex** 的生图设置里填 GPT‑6.2 Sol，会选择负责理解请求并调用工具的推理模型。OpenAI 当前文档把内置生图工具的图像模型列为 `gpt-image-2`，由 Codex 管理。 [Codex 内置生图说明](https://learn.chatgpt.com/docs/image-generation?surface=app)
- **GPT Image API** 按你在图像生成页面选的模型出图，例如 `gpt-image-2.5-flare`。完整插画边框按最终画幅选择横、竖或正方形的素材尺寸，独立装饰素材沿用设置中的尺寸。 [OpenAI 图像 API 说明](https://developers.openai.com/api/docs/guides/image-generation)

发送请求前，生成页会列出所用服务，并请你确认照片缩略图是否发送。使用现有 ChatGPT 或 Codex 登录时，接入资格与用量以各账号为准。

## 竖片选胶片框

胶片模板里会显示当前画幅和边距。选了 **1:1** 的竖片仍会出方形画布，原照片完整居中。想让成片随照片呈竖版，点预览下方的 **跟随照片方向**，看到预览合适后再应用。

35mm 半格适合在竖片的上、下保留齿孔。Super 16 单侧扫描和 65mm 五齿孔扫描会按照片方向转动片边。想要干净的暗房效果，可以选放大机手挫黑边。
