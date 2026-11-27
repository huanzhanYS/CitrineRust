//! nxym JSON 声明式插件系统。
//!
//! 服务端启动时扫描工作目录下的 `plugin/*.json`，每个文件是一份插件清单，可以声明：
//!   - `commands`: 自定义聊天命令（带或不带 `/`），玩家输入后逐个执行 `actions`；
//!   - `on_join` / `on_leave`: 玩家加入/离开时执行的 `actions`；
//!   - `chat_triggers`: 普通聊天命中关键词时执行的 `actions`（不吞掉原消息）。
//!
//! 一条 `action` 描述"给谁发什么"：`target` 为 `player`（触发者）、`broadcast`（全体）或
//! `console`（服务端日志），`text` 支持占位符 `{player}`、`{uuid}`、`{name}`、`{args}`。
//! 本模块只负责加载清单、匹配命令/关键词、展开占位符，返回 `Vec<Action>`；真正发包/广播
//! 由 `hub.rs` / `lan_actions.rs` / `lan.rs` 调用方完成（避免本模块依赖网络层）。
//!
//! 通过 `plugin/` 目录是否存在且含有效清单来激活；没有目录时整套系统静默，零开销。

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

/// 一条动作：给谁发、发什么。
#[derive(Clone, Debug, Deserialize)]
pub struct Action {
    /// `player` | `broadcast` | `console`（默认 `player`）。
    #[serde(default = "default_target")]
    pub target: String,
    pub text: String,
}

fn default_target() -> String {
    "player".to_string()
}

/// 一个自定义命令。
#[derive(Clone, Debug, Deserialize)]
pub struct Command {
    /// 命令名，不含斜杠，大小写不敏感。
    pub name: String,
    /// 别名列表。
    #[serde(default)]
    pub aliases: Vec<String>,
    /// 是否需要管理员权限（目前仅作声明，客户端命令树不注册，走聊天旁路）。
    #[serde(default)]
    pub op_only: bool,
    /// 该命令要执行的动作。
    #[serde(default)]
    pub actions: Vec<Action>,
}

/// 一个聊天关键词触发器。
#[derive(Clone, Debug, Deserialize)]
pub struct ChatTrigger {
    /// 匹配方式：`contains`（默认）| `equals` | `starts_with`。
    #[serde(default = "default_match")]
    pub r#match: String,
    /// 要匹配的文本。
    pub pattern: String,
    /// 是否大小写敏感（默认 false）。
    #[serde(default)]
    pub case_sensitive: bool,
    /// 命中后执行的动作。
    #[serde(default)]
    pub actions: Vec<Action>,
}

fn default_match() -> String {
    "contains".to_string()
}

/// 一份插件清单（对应一个 JSON 文件）。
#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub commands: Vec<Command>,
    #[serde(default)]
    pub on_join: Vec<Action>,
    #[serde(default)]
    pub on_leave: Vec<Action>,
    #[serde(default)]
    pub chat_triggers: Vec<ChatTrigger>,
}

fn default_true() -> bool {
    true
}

/// 汇总所有已加载插件的内存表示。
struct Registry {
    /// 是否加载过（OnceLock 只初始化一次）。
    manifests: Vec<Manifest>,
    /// 小写的命令名 -> 该命令的动作（合并所有插件）。
    commands: HashMap<String, Vec<Action>>,
    on_join: Vec<Action>,
    on_leave: Vec<Action>,
    chat_triggers: Vec<ChatTrigger>,
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
    REGISTRY.get_or_init(load)
}

/// 扫描 `plugin/`（相对进程工作目录，即服务端根目录）下的所有 `*.json`。
fn load() -> Registry {
    let mut reg = Registry {
        manifests: Vec::new(),
        commands: HashMap::new(),
        on_join: Vec::new(),
        on_leave: Vec::new(),
        chat_triggers: Vec::new(),
    };
    let dir = PathBuf::from("plugin");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        // 没有 plugin/ 目录：插件系统静默，不打印噪音。
        return reg;
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
        .collect();
    files.sort();
    for path in files {
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Manifest>(&text) {
                Ok(m) if m.enabled => {
                    log::info!(
                        "[nxym] 已加载插件 {} v{} ({})",
                        if m.name.is_empty() {
                            path.file_name().unwrap_or_default().to_string_lossy()
                        } else {
                            std::borrow::Cow::Owned(m.name.clone())
                        },
                        m.version,
                        path.display()
                    );
                    for cmd in &m.commands {
                        let key = cmd.name.trim().trim_start_matches('/').to_ascii_lowercase();
                        if key.is_empty() {
                            continue;
                        }
                        reg.commands
                            .entry(key.clone())
                            .or_default()
                            .extend(cmd.actions.iter().cloned());
                        for alias in &cmd.aliases {
                            let a = alias.trim().trim_start_matches('/').to_ascii_lowercase();
                            if !a.is_empty() {
                                reg.commands
                                    .entry(a)
                                    .or_default()
                                    .extend(cmd.actions.iter().cloned());
                            }
                        }
                    }
                    reg.on_join.extend(m.on_join.iter().cloned());
                    reg.on_leave.extend(m.on_leave.iter().cloned());
                    reg.chat_triggers.extend(m.chat_triggers.iter().cloned());
                    reg.manifests.push(m);
                }
                Ok(_) => log::info!("[nxym] 插件 {} 已禁用（enabled=false）", path.display()),
                Err(e) => log::warn!("[nxym] 插件 {} 解析失败，已跳过: {e}", path.display()),
            },
            Err(e) => log::warn!("[nxym] 读取插件 {} 失败: {e}", path.display()),
        }
    }
    if !reg.manifests.is_empty() {
        log::info!(
            "[nxym] 插件系统就绪：{} 个插件，{} 个命令，{} 个进服钩子，{} 个聊天触发器",
            reg.manifests.len(),
            reg.commands.len(),
            reg.on_join.len(),
            reg.chat_triggers.len()
        );
    }
    reg
}

/// 插件系统是否实际加载了任何内容。
pub fn active() -> bool {
    !registry().manifests.is_empty()
}

/// 触发强制加载（dedicated 启动时调用一次，让日志在启动时就出现而非首次事件时）。
pub fn init() {
    let _ = registry();
}

/// 展开一条动作文本里的占位符。
fn fill(text: &str, player: &str, uuid: &str, args: &str) -> String {
    text.replace("{player}", player)
        .replace("{name}", player)
        .replace("{uuid}", uuid)
        .replace("{args}", args)
}

/// 把一组原始动作展开成待发送动作（填好占位符）。
fn expand(actions: &[Action], player: &str, uuid: &str, args: &str) -> Vec<Action> {
    actions
        .iter()
        .map(|a| Action {
            target: a.target.clone(),
            text: fill(&a.text, player, uuid, args),
        })
        .collect()
}

/// 某行聊天是否是插件自定义命令（用于命令旁路放行）。
pub fn is_plugin_command(line: &str) -> bool {
    let first = line
        .trim()
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    registry().commands.contains_key(&first)
}

/// 匹配一条命令，返回要执行的动作；不是插件命令则返回空。
pub fn command_actions(line: &str, player: &str, uuid: &str) -> Vec<Action> {
    let mut parts = line.trim().trim_start_matches('/').splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or("").to_ascii_lowercase();
    let args = parts.next().unwrap_or("").trim();
    let reg = registry();
    match reg.commands.get(&first) {
        Some(actions) => expand(actions, player, uuid, args),
        None => Vec::new(),
    }
}

/// 玩家加入时执行的动作。
pub fn join_actions(player: &str, uuid: &str) -> Vec<Action> {
    expand(&registry().on_join, player, uuid, "")
}

/// 玩家离开时执行的动作。
pub fn leave_actions(player: &str, uuid: &str) -> Vec<Action> {
    expand(&registry().on_leave, player, uuid, "")
}

/// 普通聊天命中关键词时执行的动作（不吞掉原消息）。
pub fn chat_actions(message: &str, player: &str, uuid: &str) -> Vec<Action> {
    let reg = registry();
    let mut out = Vec::new();
    for t in &reg.chat_triggers {
        if t.actions.is_empty() {
            continue;
        }
        let (haystack, needle) = if t.case_sensitive {
            (message.to_string(), t.pattern.clone())
        } else {
            (message.to_ascii_lowercase(), t.pattern.to_ascii_lowercase())
        };
        let hit = match t.r#match.as_str() {
            "equals" => haystack == needle,
            "starts_with" => haystack.starts_with(&needle),
            _ => haystack.contains(&needle),
        };
        if hit {
            out.extend(expand(&t.actions, player, uuid, ""));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Manifest {
        serde_json::from_str(
            r#"{
                "name": "test", "enabled": true,
                "commands": [{"name":"rules","aliases":["规则"],"actions":[{"text":"§a规则 {args}"}]}],
                "on_join": [{"target":"broadcast","text":"{player} 加入"}],
                "chat_triggers": [{"match":"contains","pattern":"你好","actions":[{"text":"hi {player}"}]}]
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn parses_manifest_with_defaults() {
        let m = sample();
        assert_eq!(m.name, "test");
        assert!(m.enabled);
        assert_eq!(m.commands[0].aliases, vec!["规则"]);
        assert_eq!(m.on_join[0].target, "broadcast");
        assert_eq!(m.chat_triggers[0].r#match, "contains");
    }

    #[test]
    fn fills_placeholders() {
        let a = Action {
            target: "player".into(),
            text: "{player}/{uuid}/{args}/{name}".into(),
        };
        let e = expand(&[a], "Steve", "uuid-1", "foo bar");
        assert_eq!(e[0].text, "Steve/uuid-1/foo bar/Steve");
    }

    #[test]
    fn disabled_manifest_defaults_true() {
        let m: Manifest = serde_json::from_str(r#"{"name":"x"}"#).unwrap();
        assert!(m.enabled);
        assert!(m.commands.is_empty());
    }
}
