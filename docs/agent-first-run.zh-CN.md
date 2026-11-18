# Margin 新手教程：用 Codex 或 Claude 给照片加框

MCP 让 Codex 或 Claude 把你的指令交给电脑上打开的 Margin。你可以用自然语言选照片、调边框和水印、看效果，再批量导出。Margin 自带连接程序，接入时无需另外安装 Node、Pi 或配置模型 API Key。

## 1. 第一次打开 Margin

1. 打开 Margin。想用中文，点右上角的 **EN**，选 **简体中文**。
2. 拖两张适合试用的照片进窗口，或点 **导入照片 / Import photos**。在左侧单击第一张，再按住 **Shift** 点第二张，把两张都选中。
3. 点顶栏插头图标 **Agent 连接 / Agent connection**，打开 **允许本机 agent 连接 / Allow local agents to connect**。
4. 第一次可以先取消勾选 **允许调整、导入与导出 / Allow editing, importing and exporting**。显示 **已开启 · 只读 / Enabled · Read only** 后，客户端可以读取目录和预览。需要加框或导出时再勾选。

使用时保持 Margin 打开。试用版与正式版使用各自的照片目录，复制配置时以你准备操作的窗口为准。

## 2. 选一种客户端接入

### Codex 桌面端

1. 在 Margin 的 **Agent 连接**中选择 **Codex**，点 **复制配置 / Copy configuration**。
2. 在本机 Codex 中打开 **设置 → MCP servers → Add server**，名称填 `margin`，连接类型选 **STDIO**。把配置里的 `command` 和 `args` 填入对应栏位。参数依次是 `--mcp`、`--connection-file`、配置里的完整文件路径。路径即使包含空格，也要作为一个参数。
3. 保存并按提示重启客户端。在新的本机对话里输入 `/mcp` 检查 `margin` 是否可用。

习惯编辑配置文件的用户也可以把复制的 `[mcp_servers.margin]` 整段保存到 `~/.codex/config.toml`。已有同名段落时，更新那一段。若想让试用版和正式版同时可用，给其中一项取不同的名字，例如 `margin-preview`。[Codex 官方接入文档](https://learn.chatgpt.com/docs/extend/mcp?surface=desktop)

### Claude Desktop 桌面应用

1. 在 Margin 的 **Agent 连接**中选择 **通用 MCP JSON / MCP JSON**，点 **复制配置**。
2. 从 Mac 屏幕顶部的 Claude 菜单进入 **Settings → Developer → Edit Config**，打开 `~/Library/Application Support/Claude/claude_desktop_config.json`。如果是空白文件或只有 `{}`，直接粘贴整段。如果已经配过其他工具，把新配置里的 `"margin"` 项加进已有的 `"mcpServers"`，保留其他项。
3. 保存，按 **⌘Q** 完全退出 Claude，再重新打开。用输入框旁的 **＋ → Connectors** 检查是否出现 `margin`。

新版 Claude Desktop 的本机 **Code** 标签页也可读取这份配置。使用 Claude 网页版或云端会话时，这份配置无法连接电脑上的 Margin。[Claude Desktop 本地接入说明](https://modelcontextprotocol.io/docs/develop/connect-local-servers) · [Claude 的本机 Code 配置说明](https://code.claude.com/docs/en/desktop#shared-configuration) · [Anthropic 的本地与远程连接说明](https://support.claude.com/en/articles/11175166-get-started-with-custom-connectors-using-remote-mcp)

### Claude Code

在 Margin 复制 **通用 MCP JSON**。如果你在 Mac 的终端里使用 Claude Code，可以运行下面的命令，将两处中文占位替换成配置里的实际路径，保留双引号。

```sh
claude mcp add --scope user --transport stdio margin -- \
  "command 里的完整路径" --mcp --connection-file "args 里的连接文件完整路径"
claude mcp list
```

看到 `margin` 显示 **Connected** 后，在 Claude Code 中输入 `/mcp` 查看连接。这里的 `--scope user` 让本机不同项目都能使用这条连接。如果已经配好了 Claude Desktop，也可以用 `claude mcp add-from-claude-desktop`，在提示中只选择 `margin`。[Claude Code 官方接入文档](https://code.claude.com/docs/en/mcp)

## 3. 用一组照片走完流程

**先确认连接。** 在 Codex 或 Claude 中发送这句话。只读模式就能完成。

> 请调用 Margin 的 `margin_get_workspace`。告诉我导入了几张照片、选中了几张，以及当前显示的照片是哪张。

**试着加框。** 回到 Margin 勾选 **允许调整、导入与导出**。想给两张一起加框，确认选区里有两张，并在新版侧栏选中 **编辑选中 2 张 / Edit 2 selected**。发送这句话后可以直接在 Margin 窗口看效果。

> 请用 Margin 给我选中的两张照片加 8% 的奶油色边框，设为 4:5，并在照片右下角加小号文字水印「我的署名」。设置完成后，把当前照片留在 Margin 窗口给我检查，等我确认再导出。

**确认后导出。** 在 Finder 新建一个测试导出文件夹，把它的完整路径填到下方指令里。

> 用 Margin 只导出当前选中的两张照片。保存为 JPG，质量 95，放到「你的导出文件夹的完整路径」。等处理结束后告诉我成功几张，并列出生成的文件。

在 Finder 核对两张成片。Margin 会另存副本，重名时自动编号，原图保留。

## 4. 遇到问题时

| 看到的情况 | 怎么排查 |
| --- | --- |
| 找不到 `margin` | 确认 Margin 正开着、Agent 连接已开启，保存配置后重启客户端。如果移动了 App 或切换到试用版，从当前版本重新复制配置。 |
| 能看到照片数量，但改不了边框 | 检查 **允许调整、导入与导出** 是否开启，并关闭 Margin 中尚未处理的对话框。 |
| Agent 连到另一版 Margin | 核对注册时的程序路径和连接文件路径是否来自目标版本。并行使用时给两项取不同的名字。 |
| 导出请求超时 | 先看 Margin 的导出进度，或请 agent 用 `margin_get_export_status` 检查本批次，再决定是否重试。 |
| 通过 agent 导入时无法读取个别照片 | 先在 Margin 用 **导入照片** 的文件选择器选中原图，再让 agent 操作已导入的照片。 |

Margin 在本机处理照片。若让 agent 调用 `margin_get_preview` 并在聊天里显示图片，当前成片的缩小预览会交给已连接的客户端。想只在本机看图时，就在 Margin 窗口检查效果。用完可以关闭 **允许本机 agent 连接**。

[完整工具列表与协议说明](agent-mcp-0.11.3.md)
