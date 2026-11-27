# Build from source / 从源码构建

本仓库根目录是**开箱即用的配置与文档**；完整的编译二进制（116 MB）与 26.3 游戏资源（322 MB）超过 GitHub 单文件 100 MB 限制，请从 [Releases](https://github.com/huanzhanYS/CitrineRust/releases) 下载 `CitrineRust-github.zip`，解压即用。

如果你想自己在 [Horizon](https://github.com/horizon-rust/horizon) 源码上复现 CitrineRust 的定制功能，本目录提供了补丁：

| 文件 | 说明 |
|---|---|
| `citrine.patch` | 相对上游 Horizon 源码的完整 diff（用 `git apply` 应用） |
| `accounts.rs` | 新增模块：登录/注册系统（SHA256 + 盐） |
| `plugin.rs` | 新增模块：JSON 声明式插件系统 |

## 定制功能 / What's changed

1. **nxym 启动横幅**：在 `game/dedicated.rs` 启动时打印 ASCII art。
2. **登录/注册系统**：新增 `accounts.rs`；`Guest` 加 `authenticated` 状态；未登录玩家被冻结，只能用 `register`/`login`/`changepassword`；账号存 `config/nxym_users.json`。
3. **JSON 插件系统**：新增 `plugin.rs`；启动时扫描 `plugin/*.json`；支持自定义命令、`on_join`/`on_leave`、聊天关键词触发。用 Cargo feature `nxym-plugin` 门控。
4. **多线程区块生成**：`start.sh` 设置 `HORIZON_JOB_THREADS` / `HORIZON_WORKER_THREADS`，`server.toml` 开 `parallel_tick` 与 `horizon-pregen`。

## 应用补丁 / Apply

> 补丁基于社区 fork 的 `android-port` 分支源码（见 README）。由于上游与 fork 会有漂移，应用时可能需要手工调整上下文。

```bash
# 在 Horizon 源码根目录
git apply /path/to/citrine.patch
# 把 accounts.rs / plugin.rs 放到 crates/horizon-client/src/
# 用 Rust 1.95 编译
cargo +1.95.0 build --release --no-default-features --features nxym-plugin --bin horizon-client --bin horizon-server
```

## 许可 / License

MIT © huanzhanYS & CitrineRust contributors；基于 Horizon © Ludovic Marie & Horizon contributors (MIT)。详见根目录 `LICENSE`。
