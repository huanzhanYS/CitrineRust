# CitrineRust · JSON 声明式插件 / Declarative JSON Plugins

本目录里的每个 `*.json` 文件就是一个插件。服务端**启动时自动扫描**本目录，加载所有 `enabled: true` 的插件，无需改源码、无需重编译。删掉某个 `.json` 或设 `enabled: false` 即卸载。

> 本目录当前为空（无示例插件）。内置示例插件（welcome / commands / chat-triggers）见**用户版** `CitrineRust.zip`。两个版本的插件系统与二进制完全一致，差别仅在于本目录是否预置示例插件。可按下方格式自行添加 `*.json`。

## 快速开始

1. 复制 `commands.json` 为 `myplugin.json`
2. 修改内容
3. 重启服务端（或 `./start.sh restart`）

启动时控制台会打印：
```
[nxym] 已加载插件 welcome v1.0.0 (plugin/welcome.json)
[nxym] 插件系统就绪：3 个插件，N 个命令，5 个进服钩子，3 个聊天触发器
```

## 插件 JSON 格式

```jsonc
{
  "name": "myplugin",          // 插件名（可选）
  "version": "1.0.0",          // 版本（可选）
  "description": "...",        // 描述（可选）
  "enabled": true,             // 是否启用，默认 true

  // 自定义命令：玩家输入 /xxx 或裸词 xxx 都能触发
  "commands": [
    {
      "name": "shop",          // 命令名（不含斜杠，大小写不敏感）
      "aliases": ["商店"],      // 别名（可选）
      "op_only": false,        // 声明用，当前不强制
      "actions": [ /* 动作列表 */ ]
    }
  ],

  // 玩家进服时执行的动作
  "on_join": [ /* 动作列表 */ ],

  // 玩家离开时执行的动作
  "on_leave": [ /* 动作列表 */ ],

  // 聊天关键词触发（不吞掉原消息）
  "chat_triggers": [
    {
      "match": "contains",     // contains(默认) | equals | starts_with
      "pattern": "你好",        // 匹配的文本
      "case_sensitive": false, // 是否大小写敏感，默认 false
      "actions": [ /* 动作列表 */ ]
    }
  ]
}
```

## 动作（Action）

每个动作描述"给谁发、发什么"：

```jsonc
{
  "target": "player",     // player=触发者(默认) | broadcast=全体在线 | console=服务端日志
  "text": "§a欢迎 {player}！"
}
```

## 占位符

`text` 里可用的占位符：

| 占位符 | 含义 |
|---|---|
| `{player}` 或 `{name}` | 玩家名 |
| `{uuid}` | 玩家 UUID |
| `{args}` | 命令参数（仅 `commands` 会用；如 `/echo 你好` 里 `{args}` = `你好`） |

颜色用 Minecraft 的 `§` 颜色码（如 `§a` 绿、`§c` 红、`§e` 黄、`§f` 白、`§7` 灰）；`console` 目标会自动去掉颜色码。

## 完整示例

```json
{
  "name": "greeter",
  "enabled": true,
  "on_join": [
    { "target": "broadcast", "text": "§e{player} 进服了！" },
    { "target": "player", "text": "§a欢迎，{player}！你的 UUID 是 {uuid}" }
  ],
  "commands": [
    {
      "name": "hi",
      "aliases": ["你好"],
      "actions": [
        { "target": "player", "text": "§a你好呀 {player}，你说的是: {args}" }
      ]
    }
  ],
  "chat_triggers": [
    {
      "match": "contains",
      "pattern": "GG",
      "case_sensitive": false,
      "actions": [{ "target": "broadcast", "text": "§6{player}: GG!" }]
    }
  ]
}
```

## 注意事项

- **认证优先**：未登录玩家只能用 `/register`、`/login`，插件命令对未登录玩家不生效（防绕过认证）。
- **命令不进客户端命令树**：插件命令走聊天旁路，玩家输入时不带 Tab 补全提示，但带不带 `/` 都能触发。
- **表格驱动、无脚本**：插件只做"匹配 → 发消息"，不能执行任意代码。需要更复杂的逻辑（如权限组、计分板联动）需改源码或另写外部守护。
- **热加载**：当前需重启生效；如需热加载可执行 `./start.sh restart`。
