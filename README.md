# CitrineRust · Minecraft Java 26.3 Server (Rust Reimplementation)

> **[EN]** **CitrineRust** is a ready-to-run, self-contained server distribution built on [Horizon](https://github.com/horizon-rust/horizon) — a from-scratch Rust reimplementation of the Minecraft Java 26.3 server (protocol 777). It bundles the compiled headless engine, prepared 26.3 game resources, multithreaded chunk generation, a built-in login/registration system, Chunky-style chunk pre-generation, and a declarative JSON plugin system. Unzip and run — no Java, no recompilation required.

> **[中文]** **CitrineRust** 基于 [Horizon](https://github.com/horizon-rust/horizon)（从零重写的 Minecraft Java 26.3 服务端，协议 777），是一个开箱即用的自包含发行版：内置编译好的无头引擎、26.3 游戏资源、多线程区块生成、登录/注册系统、Chunky 式区块预生成和声明式 JSON 插件系统。解压即用，无需 Java、无需重编译。

> **版本 Edition**：GitHub 公开版（纯净骨架）— 保留 nxym 启动横幅、内置登录/注册、区块预生成与 JSON 插件系统；`plugin/` 目录为空（仅含开发文档），由使用者自行编写插件。开箱即用的示例插件请用用户版。

> **许可 License**：MIT © huanzhanYS & CitrineRust contributors；基于 Horizon © Ludovic Marie & Horizon contributors (MIT)。`resources/` 内的 Minecraft 数据归 Mojang/Microsoft 所有，请遵守其 EULA。

## 快速开始 / Quick Start

```bash
cd CitrineRust
./start.sh start      # 后台启动
./start.sh status     # 查看状态（TPS / MSPT / 玩家 / 内存）
./start.sh console    # 进入控制台（Ctrl-D 退出控制台但不停服）
./start.sh stop       # 停止
```

客户端连接：Minecraft Java **26.3** → 多人游戏 → 服务器地址填 `127.0.0.1:25565`（局域网其他机器填本机 IP）。协议号 777。

其他命令：`./start.sh {restart|logs|run|cmd}`。`cmd` 可直接下发控制台命令，例如 `./start.sh cmd list`。

## 多线程区块生成（已启用）

Horizon 把区块生成、光照、meshing 放进一个全局 worker 池（`horizon-threads::jobs`），本包已显式放大并固化：

| 开关 | 位置 | 作用 |
|---|---|---|
| `HORIZON_JOB_THREADS` | `start.sh` 自动设为 CPU 核数 | 区块生成 worker 数（优先级最高，专控该池） |
| `HORIZON_WORKER_THREADS` | `start.sh` 自动设为 CPU 核数 | 全局 worker 核数上限（并行 tick 等阶段） |
| `parallel_tick = true` | `server.toml [engine]` | tick 的可并行阶段分摊给 helper 线程（自动生成 `HORIZON_PARALLEL_TICK=1`） |
| `horizon-pregen = true` | `server.toml [properties]` | 开启 `/pregen` 多线程区块预生成命令 |

默认值取机器逻辑核数（`getconf _NPROCESSORS_ONLN`）。想手动指定线程数，用环境变量覆盖：

```bash
HORIZON_JOB_THREADS=16 HORIZON_WORKER_THREADS=16 ./start.sh start
```

启动日志会打印实际 worker 数（`Horizon: ... N worker threads`）。实测：设为 16 时游戏进程出现 16 个 `horizon-worker-` 线程；一次 1089 区块的 `/pregen` 预生成约 8 秒完成（约 139 chunks/s），确认多核并行生效。

多线程预生成用法（可选，提前生成地形，玩家探索时更流畅）：

```bash
./start.sh cmd pregen start 2500      # 以出生点为中心预生成半径 2500 方格（约 9.8 万区块）
./start.sh cmd pregen status          # 进度
./start.sh cmd pregen cancel          # 取消（已生成区块保留）
```

> 注意：`parallel_tick` 与 `/pregen` 产生的世界随机序列与单线程原版不完全一致（生物生成、部分随机取值不同），概率与规则不变，但同一种子的具体结果会不同。这是并行化的固有差异，见源码 `docs/parallel-natural-spawning.md`、`docs/pregeneration.md`。

## nxym 内置功能（登录/注册 + Chunky 预加载）

本包的 `nxym` 功能已**直接编译进服务端二进制**（改自 Horizon 源码），不再是外部插件，也无需启动 Python 守护进程。启动横幅 `nxym` ASCII art、登录/注册系统、Chunky 式区块预生成都在引擎里原生运行，开箱即用。

**登录/注册系统**：玩家进入世界时处于未认证状态——**不能移动、聊天、破坏方块或交互**，只能输入以下命令（在游戏内聊天框，带不带 `/` 均可）：

| 输入 | 作用 |
|---|---|
| `/register <密码>` 或 `register <密码>` | 注册并自动登录（密码至少 4 位） |
| `/login <密码>` 或 `login <密码>` | 登录（已注册玩家） |
| `/changepassword <旧> <新>` | 修改密码 |

进服会收到提示，告知该用 register 还是 login。认证成功后即恢复正常游戏。账号存在 `config/nxym_users.json`，密码用 **SHA256 + 随机盐** 加盐哈希（不明文存储），格式与旧插件完全兼容，可直接沿用已有账号；玩家掉线后再次进入需重新登录。

**Chunky 预加载** 为源码内置的 `/pregen` 命令（五阶段多线程流水线：生成→装饰→完成→光照→保存），可用两种方式发起：

```bash
./start.sh pregen 2500 square          # 快捷入口：以出生点为中心预生成半径 2500 方格
./start.sh cmd pregen start 2500 circle  # 控制台原生命令
./start.sh cmd pregen status           # 进度
./start.sh cmd pregen pause|continue|cancel
```

> 说明：早期版本用外部 Python 插件（`nxym_plugin.py`）通过控制 API 实现这些功能；现已全部内置到二进制，插件文件仅作历史留档保留，日常运行**不需要**它。

## JSON 声明式插件 / JSON Plugins

本包带一套 JSON 声明式插件系统（两版共有）：服务端启动时自动扫描 `plugin/*.json`，加载所有启用的插件。新增插件只需放一个 JSON 文件并重启，无需改源码或重编译。

- 可声明：自定义命令（`/xxx` 或裸词）、进服/离服欢迎语、聊天关键词自动回复
- 动作目标：`player`（触发者）、`broadcast`（全体）、`console`（日志）
- 占位符：`{player}`、`{uuid}`、`{args}`；颜色用 `§` 码
- 本版本 `plugin/` 目录为空（无示例插件），可自行按文档添加；开箱即用的 3 个示例插件见用户版

详见 [`plugin/README.md`](plugin/README.md)。未登录玩家只能用 register/login，插件命令对其不生效（防绕过认证）。

> **两个版本**：`CitrineRust.zip`（用户版）含插件系统 + `plugin/` 下 3 个开箱即用的示例插件；`CitrineRust-github.zip`（本 GitHub 版）同样保留插件系统与 `plugin/` 目录，但**不含示例插件**，适合公开发布、由使用者自行编写插件。两版二进制与核心功能完全一致。

## 面板托管

服务端**自带 Web 面板**（`./start.sh panel` 取登录链接，仅 127.0.0.1:8130）。若要用 Pterodactyl/Pelican 等面板托管：官方无现成 Egg，但本包 `panel/egg-pterodactyl.json` 提供了可导入的示例，启动命令用 `./start.sh run`（前台模式）。详见 `panel/README.md`。

## 目录内容

| 文件 | 说明 |
|---|---|
| `horizon-client` | 游戏引擎，headless 专用（`--dedicated`），约 111 MB |
| `horizon-server` | 监督器 / CLI（启动、状态、控制台、备份、MCP 等） |
| `horizon-prepare` | 资源准备工具（一般无需再用；资源已就绪） |
| `resources/` | 从正版 26.3 提取的 prepared 资源（约 322 MB） |
| `server.toml` | 主配置（已开启多线程） |
| `server.properties` | 兼容原版键；启动时由 server.toml 覆写，改配置请编辑 server.toml |
| `survival/` | 世界存档 |
| `start.sh` | 一键管理脚本（多线程环境变量、pregen 快捷入口） |
| `plugin/` | JSON 声明式插件目录（放 `*.json` 即生效；本版仅含开发文档，详见 `plugin/README.md`） |
| `config/nxym_users.json` | 注册账号数据库（SHA256 + 盐，加盐哈希，格式兼容旧插件） |
| `nxym_plugin.py` | 旧版外部插件（历史留档，功能已内置，无需运行） |
| `panel/` | 面板托管说明 + Pterodactyl Egg 示例 |
| `config/`、`logs/` | 配置文件与日志（含 `nxym-plugin.log`） |

Web 管理面板默认 `http://127.0.0.1:8130`（仅本机）。控制 API 地址写在 `server.toml [api]`。

## 配置与安全

- 当前为**离线模式**（`online_mode = false`）：任何人可用任意昵称加入，仅建议在可信网络使用。公网部署请在 `server.toml` 改 `online_mode = true` 并按需开白名单。
- 改端口：编辑 `server.toml [server] port`。改视距：`view_distance` / `simulation_distance`。
- 自动备份：`[backups] every = "6h"`、`keep = 12`，也可 `./start.sh backup`。

## 法律须知

`resources/` 内的 26.3 数据由 Horizon 的资源准备工具从**正版 Minecraft Java 26.3** 提取生成，仅用于运行该开源实现。Minecraft 商标与内容归 Mojang/Microsoft 所有；请自行持有正版副本并遵守其最终用户许可协议（EULA）。Horizon 为社区开源项目，非 Mojang 官方产品。

## 环境要求

Linux x86_64。二进制在 **Ubuntu 20.04（glibc 2.31）** 编译，最高仅需 `GLIBC_2.30`，兼容 Ubuntu 20.04/22.04/24.04、Debian 11+ 及绝大多数面板容器（如 Pterodactyl/Pelican、简幻欢等）。若旧版曾报 `version 'GLIBC_2.39' not found`，请用本修复版。

运行需要的系统库：`libudev`、`libasound2`、`libxkbcommon`、`pkg-config`。若启动报 `error while loading shared libraries`，安装对应库即可，例如：

```bash
sudo apt-get install -y libudev1 libasound2 libxkbcommon0
```
