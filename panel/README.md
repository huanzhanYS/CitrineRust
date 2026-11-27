# 面板服务器能否托管本服务端？

调查结论（2026-11-27）：**可以用面板托管，但没有官方现成 Egg，需要自制一个（本目录已提供示例）。**

## 结论速览

| 面板 | 能否用 | 说明 |
|---|---|---|
| **Pterodactyl / Pelican** | ✅ 可以（需自制 Egg） | 面板靠「Egg」模板运行任意服务端；Horizon 无官方 Egg，但本目录 `egg-pterodactyl.json` 已给出可用示例 |
| **Horizon 自带 Web 面板** | ✅ 已内置 | 服务端自带 `http://127.0.0.1:8130`，开箱即用（TPS/MSPT/内存/CPU 图表、控制台、玩家管理） |
| **MCSMyAdmin / Crafty Controller / AMP 等** | ✅ 一般可以 | 这类面板支持「自定义启动命令」，填 `./start.sh run` 即可 |
| 各类「一键开服」国内面板（含 Java 环境） | ⚠️ 视情况 | 多数只认 jar 文件；本服务端是原生二进制，需面板支持自定义可执行程序 |

## 为什么 Pterodactyl 能用

Pterodactyl 通过 **Egg（蛋）** 机制支持游戏：一个 Egg 本质是「启动命令 + 环境变量 + Docker 镜像 + 文件模板」的配置。官方 Egg 只覆盖 Vanilla/Paper/Forge 等常见 Java 服务端，**没有 Horizon 的 Egg**；但官方文档明确：*「理论上只要会写启动脚本，任何服务端程序都能包装成 Egg 接进来」*，社区 Egg 仓库里甚至有 Node.js/Python 机器人。所以：

- **关键适配点**：面板要求进程保持在前台运行，本服务端用 `horizon-server run`（`start.sh run` 里的 `exec`）正好是前台模式，符合面板要求。
- **Docker 依赖**：官方未发布 Docker 镜像（`hosting.md` 明确写 "No Docker image is published"），但可用 Pterodactyl 自带的通用 Java/基础镜像（本服务端只需 glibc + libudev/libasound2/libxkbcommon，实际不跑 Java）。若镜像缺这三个库，用自定义镜像 `apt install libudev1 libasound2 libxkbcommon0` 即可。
- **端口**：主端口 25565（TCP）；Web 面板 8130 默认仅本机；RCON 可选。Egg 里主要声明 25565。
- **线程数**：用 Egg 变量把 `HORIZON_JOB_THREADS` / `HORIZON_WORKER_THREADS` 暴露出来，建议设为容器 CPU 限额（见示例 Egg）。

## 使用方法

1. Pterodactyl 后台 → **Eggs → Import Egg**，粘贴 `egg-pterodactyl.json`。
2. 新建服务器，选择该 Egg，分配 CPU/内存（建议 2 核 / 2GB 起）。
3. 上传本服务端包全部文件（zip 解压后的内容）到服务器根目录。
4. 启动命令：`./start.sh run`（Egg 里已设好）。
5. 如容器缺系统库，在自定义镜像中安装 `libudev1 libasound2 libxkbcommon0`。

## 备选：不用面板，直接用内置面板

如果只想图形化管理单个服务器，不必上 Pterodactyl——本服务端自带 Web 面板：

```bash
./start.sh start
./start.sh panel     # 打印一次性登录链接 (10 分钟有效)
```

浏览器打开该链接即可看到实时 TPS/MSPT/内存/CPU、控制台、玩家列表、备份。默认只监听 `127.0.0.1:8130`，安全（面板不可暴露公网）。

## 参考来源

- Pterodactyl 官方文档 <https://docs.pterodactyl.io/v2/project>：Egg 机制、支持的游戏、社区 Egg 仓库
- Horizon `docs/hosting.md`：无官方 Docker 镜像、自带 Web 面板、前台 `run` 模式
