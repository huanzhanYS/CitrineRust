//! nxym 登录 / 注册系统（类似 AuthMe），内置于服务端本体。
//!
//! 玩家进入世界时处于"未认证"状态：不能移动、聊天、破坏方块或交互，只能使用
//! `/register`、`/login`（也接受不带斜杠的聊天文本，兼容旧插件用法）。
//! 账号以小写玩家名为主键存在 `config/nxym_users.json`，格式与旧版 nxym 插件
//! 完全一致（`sha256$<salt>$<digest>`），因此可以直接沿用已有账号数据。
//!
//! 这里只处理账号数据与口令校验，返回给调用方要发给玩家的消息文本；网络包、状态
//! 冻结与命令分发分别由 `hub.rs`、`lan_actions.rs`、`lan.rs` 负责。

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// nxym 服务端启动横幅，编译进二进制；dedicated 启动时逐行打印。
pub const BANNER: &[&str] = &[
    "",
    " ███╗   ██╗██╗  ██╗██╗   ██╗███╗   ███╗",
    " ████╗  ██║╚██╗██╔╝╚██╗ ██╔╝████╗ ████║",
    " ██╔██╗ ██║ ╚███╔╝  ╚████╔╝ ██╔████╔██║",
    " ██║╚██╗██║ ██╔██╗   ╚██╔╝  ██║╚██╔╝██║",
    " ██║ ╚████║██╔╝ ██╗   ██║   ██║ ╚═╝ ██║",
    " ╚═╝  ╚═══╝╚═╝  ╚═╝   ╚═╝   ╚═╝     ╚═╝",
    "",
    "        N X Y M  S E R V E R   ·  Minecraft Java 26.3",
    "        Powered by Horizon (Rust)  |  protocol 777",
    "        Multithread chunkgen / Auth / Chunky pregen built in",
    "",
];

/// 一条账号记录，字段与旧插件 `config/nxym_users.json` 一致。
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Account {
    /// `sha256$<salt>$<digest>`，`digest = sha256(salt + password)`。
    salt_hash: String,
    #[serde(default)]
    ip: String,
    #[serde(default)]
    registered: i64,
    #[serde(default)]
    last_login: i64,
}

/// 账号库：文件路径 + 小写名到记录的映射。
struct Store {
    file: PathBuf,
    accounts: HashMap<String, Account>,
}

struct Auth {
    /// dedicated 服务端且开启了 nxym 认证时为 true。
    enabled: AtomicBool,
    /// 账号库，第一次使用时从磁盘加载。
    store: Mutex<Option<Store>>,
    /// 生成盐用的计数器。
    nonce: AtomicU64,
}

static AUTH: OnceLock<Auth> = OnceLock::new();

fn auth() -> &'static Auth {
    AUTH.get_or_init(|| Auth {
        enabled: AtomicBool::new(false),
        store: Mutex::new(None),
        nonce: AtomicU64::new(0),
    })
}

use std::sync::OnceLock;

/// dedicated 服务端启动时调用：启用认证并加载账号库。
pub fn enable() {
    let a = auth();
    a.enabled.store(true, Ordering::Relaxed);
    let mut store = a.store.lock().unwrap_or_else(|e| e.into_inner());
    if store.is_none() {
        *store = Some(Store::open());
    }
}

/// 认证系统是否启用。
pub fn enabled() -> bool {
    auth().enabled.load(Ordering::Relaxed)
}

fn key(name: &str) -> String {
    name.to_ascii_lowercase()
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl Store {
    /// 读取 `config/nxym_users.json`；不存在或损坏时从空库开始。
    fn open() -> Self {
        let file = PathBuf::from("config").join("nxym_users.json");
        let mut accounts = HashMap::new();
        if let Ok(text) = std::fs::read_to_string(&file) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, Account>>(&text) {
                for (name, acc) in map {
                    accounts.insert(key(&name), acc);
                }
            } else {
                log::warn!("nxym: config/nxym_users.json 格式无法识别，从空账号库开始");
            }
        }
        Store { file, accounts }
    }

    /// 写回磁盘，先写临时文件再替换，避免中途崩溃损坏账号库。
    fn save(&self) {
        if let Some(parent) = self.file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = self.file.with_extension("json.tmp");
        match serde_json::to_string_pretty(&self.accounts) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&tmp, text) {
                    log::error!("nxym: 保存账号库失败: {e}");
                    return;
                }
                if let Err(e) = std::fs::rename(&tmp, &self.file) {
                    log::error!("nxym: 替换账号库失败: {e}");
                }
            }
            Err(e) => log::error!("nxym: 序列化账号库失败: {e}"),
        }
    }
}

fn new_salt() -> String {
    let a = auth();
    let t = now_millis() as u64;
    let n = a.nonce.fetch_add(1, Ordering::Relaxed);
    // 足够作为盐：时间戳 + 计数器 + 地址混合，格式保持十六进制。
    let mut h = Sha256::new();
    h.update(t.to_le_bytes());
    h.update(n.to_le_bytes());
    h.update((a as *const Auth as usize).to_le_bytes());
    let d = h.finalize();
    d[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

fn hash_pw(salt: &str, password: &str) -> String {
    let mut h = Sha256::new();
    h.update(salt.as_bytes());
    h.update(password.as_bytes());
    format!("{:x}", h.finalize())
}

fn check_pw(password: &str, stored: &str) -> bool {
    let mut parts = stored.split('$');
    let (Some(_), Some(salt), Some(digest)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    hash_pw(salt, password) == digest
}

/// 该玩家是否已注册（供加入时判断用）。
pub fn is_registered(name: &str) -> bool {
    let a = auth();
    let store = a.store.lock().unwrap_or_else(|e| e.into_inner());
    store
        .as_ref()
        .is_some_and(|s| s.accounts.contains_key(&key(name)))
}

/// 处理一行认证命令（`register` / `login` / `changepassword`，可带前导 `/`）。
/// 返回 (发给玩家的消息, 认证是否成功)；认证成功时调用方应清除该玩家的冻结状态。
pub fn handle_line(name: &str, ip: &str, line: &str) -> (Vec<String>, bool) {
    let words: Vec<&str> = line
        .trim()
        .trim_start_matches('/')
        .split_whitespace()
        .collect();
    let Some(command) = words.first() else {
        return (Vec::new(), false);
    };
    let command = command.to_ascii_lowercase();
    let k = key(name);
    let a = auth();
    let mut guard = a.store.lock().unwrap_or_else(|e| e.into_inner());
    let store = guard.get_or_insert_with(Store::open);

    match command.as_str() {
        "register" => {
            if store.accounts.contains_key(&k) {
                return (vec!["§c你已经注册过了，请使用 /login <密码> 登录".to_string()], false);
            }
            let Some(password) = words.get(1) else {
                return (vec!["§e用法: /register <密码>".to_string()], false);
            };
            if password.len() < 4 {
                return (vec!["§c密码至少需要 4 个字符".to_string()], false);
            }
            let salt = new_salt();
            let acc = Account {
                salt_hash: format!("sha256${salt}${}", hash_pw(&salt, password)),
                ip: ip.to_string(),
                registered: now_millis(),
                last_login: 0,
            };
            store.accounts.insert(k, acc);
            store.save();
            let msg = format!(
                "§a[ nxym ] 注册成功！你现在可以正常游戏了。请牢记密码：§f{password}"
            );
            log::info!("[nxym] {name} 注册了账号");
            (vec![msg], true)
        }
        "login" => {
            let Some(password) = words.get(1) else {
                return (vec!["§e用法: /login <密码>".to_string()], false);
            };
            let Some(acc) = store.accounts.get_mut(&k) else {
                return (
                    vec!["§c你还没有注册，请先输入 /register <密码>".to_string()],
                    false,
                );
            };
            if !check_pw(password, &acc.salt_hash) {
                return (vec!["§c密码错误，请重试".to_string()], false);
            }
            acc.last_login = now_millis();
            acc.ip = ip.to_string();
            store.save();
            log::info!("[nxym] {name} 登录成功");
            (vec!["§a[ nxym ] 登录成功，欢迎回来！".to_string()], true)
        }
        "changepassword" => {
            let (Some(old), Some(new)) = (words.get(1), words.get(2)) else {
                return (vec!["§e用法: /changepassword <旧密码> <新密码>".to_string()], false);
            };
            if new.len() < 4 {
                return (vec!["§c新密码至少需要 4 个字符".to_string()], false);
            }
            let Some(acc) = store.accounts.get_mut(&k) else {
                return (vec!["§c你还没有注册".to_string()], false);
            };
            if !check_pw(old, &acc.salt_hash) {
                return (vec!["§c旧密码错误".to_string()], false);
            }
            let salt = new_salt();
            acc.salt_hash = format!("sha256${salt}${}", hash_pw(&salt, new));
            store.save();
            log::info!("[nxym] {name} 修改了密码");
            (vec!["§a[ nxym ] 密码已修改".to_string()], false)
        }
        _ => (Vec::new(), false),
    }
}

/// 该行文本是否是一条认证命令（register/login/changepassword）。
pub fn is_auth_command(line: &str) -> bool {
    let first = line.trim().trim_start_matches('/').split_whitespace().next();
    matches!(
        first.map(str::to_ascii_lowercase).as_deref(),
        Some("register" | "login" | "changepassword")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_format_matches_legacy_plugin_and_verifies() {
        let salt = "0123456789abcdef";
        let stored = format!("sha256${salt}${}", hash_pw(salt, "secret123"));
        // 旧插件格式: sha256$<salt>$<sha256(salt + password) hex>，三段、盐为十六进制。
        let parts: Vec<&str> = stored.split('$').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "sha256");
        assert_eq!(parts[1], salt);
        assert_eq!(parts[2].len(), 64); // SHA-256 十六进制
        assert!(parts[2].chars().all(|c| c.is_ascii_hexdigit()));
        assert!(check_pw("secret123", &stored));
        assert!(!check_pw("wrong", &stored));
    }

    #[test]
    fn different_salts_produce_different_hashes_for_same_password() {
        assert_ne!(hash_pw("salt-a", "same"), hash_pw("salt-b", "same"));
    }

    #[test]
    fn bad_stored_hash_is_rejected() {
        assert!(!check_pw("x", "garbage"));
        assert!(!check_pw("x", "sha256$onlysalt"));
    }

    #[test]
    fn auth_command_recognises_bare_and_slash_and_case() {
        for line in [
            "register pw",
            "/register pw",
            "LOGIN pw",
            "/Login pw",
            " changepassword a b ",
        ] {
            assert!(is_auth_command(line), "{line}");
        }
        for line in ["list", "/help", "hello register", "", "say hi"] {
            assert!(!is_auth_command(line), "{line}");
        }
    }

    #[test]
    fn names_are_case_insensitive() {
        assert_eq!(key("Steve"), key("steve"));
        assert_eq!(key("Alex"), "alex");
    }
}
