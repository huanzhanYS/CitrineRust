# CitrineRust

> A ready-to-run Minecraft Java 26.3 server, reimagined in Rust.
> 一个用 Rust 重写、开箱即用的 Minecraft Java 26.3 服务端。

**[EN]** CitrineRust is a self-contained, ready-to-run distribution of **Horizon** — a from-scratch Rust reimplementation of the Minecraft Java 26.3 server (protocol 777). It bundles the compiled headless engine, prepared 26.3 game resources and multithreaded chunk generation, plus built-in features added on top of upstream Horizon: a custom `nxym` startup banner, a login/registration system, Chunky-style chunk pre-generation, and a declarative JSON plugin system. Just unzip and run — no Java runtime, no recompilation, no game client needed.

**[中文]** CitrineRust 是基于 **Horizon**（从零重写的 Minecraft Java 26.3 服务端，协议 777）的自包含开箱即用发行版。它内置编译好的无头引擎、26.3 游戏资源与多线程区块生成，并在上游 Horizon 之上加入：自定义 `nxym` 启动横幅、登录/注册系统、Chunky 式区块预生成，以及声明式 JSON 插件系统。解压即用 —— 无需 Java、无需重编译、无需游戏客户端。

---

## Highlights / 特性

- **Rust 原生性能 / Native Rust performance** — 低内存占用、20 TPS、多线程区块生成（生成/光照/meshing 并行）。
- **登录注册 / Auth system** — 未登录玩家被冻结；密码 SHA256 + 随机盐存储，兼容旧格式。
- **区块预生成 / Chunk pregen** — 内置 Chunky 式 `/pregen` 五阶段流水线，支持 `start/status/pause/cancel`。
- **JSON 插件 / JSON plugins** — 放一个 `*.json` 即扩展自定义命令、进服/离服广播、聊天关键词回复；`{player}`/`{uuid}`/`{args}` 占位符。
- **Web 面板 / Web panel** — 自带 127.0.0.1:8130 管理面板，并提供 Pterodactyl/Pelican Egg 示例。

## Quick start / 快速开始

```bash
cd CitrineRust
./start.sh start     # 后台启动 / start in background
./start.sh status    # 状态 / status
./start.sh stop      # 停止 / stop
```

客户端连接 / Connect: Minecraft Java **26.3** → `127.0.0.1:25565`（协议 777）。进服先 `/register <密码>` 注册、`/login <密码>` 登录。

## Editions / 版本

| 版本 Edition | 文件 File | 说明 Description |
|---|---|---|
| 用户版 User | `CitrineRust.zip` | 含 3 个示例插件 / with 3 example plugins |
| GitHub 版 | `CitrineRust-github.zip` | `plugin/` 为空骨架 / empty plugin skeleton |

两版二进制与核心功能完全一致 / Same binary and core features.

## License

MIT © huanzhanYS & CitrineRust contributors. Based on [Horizon](https://github.com/horizon-rust/horizon) © Ludovic Marie & Horizon contributors (MIT).

Minecraft is a trademark of Mojang/Microsoft. `resources/` contains data extracted from a legitimate copy of Minecraft Java 26.3; please comply with the Mojang EULA. Not an official Mojang product.
