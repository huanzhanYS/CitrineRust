#!/usr/bin/env bash
# nxym · Horizon (Minecraft Java 26.3, Rust 重写) 服务端一键管理脚本
# 用法: ./start.sh {start|stop|restart|status|console|logs|run|cmd|pregen}
# 启动横幅、登录/注册、区块预生成均已编译进服务端本体，无需外部插件。
set -e

DIR="$(cd "$(dirname "$0")" && pwd)"
SERVER_BIN="$DIR/horizon-server"

if [ ! -x "$SERVER_BIN" ]; then
  echo "错误: 找不到 $SERVER_BIN，请确认压缩包已完整解压"
  exit 1
fi

# ---------- 多线程区块生成配置 ----------
# Horizon 的区块生成/光照/meshing 共用一个全局 worker 池（horizon-threads::jobs）。
# 默认线程数 = CPU 核数 - 1；这里显式放大，让区块生成真正并行跑满核心。
#   HORIZON_JOB_THREADS   : 区块生成 worker 数（优先级最高，专控该池）
#   HORIZON_WORKER_THREADS: 全局 worker 核数上限（par_chunks_mut 等并行阶段）
# 可用环境变量覆盖，例如: HORIZON_JOB_THREADS=16 ./start.sh start
PHYS_CORES="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"
JOB_THREADS="${HORIZON_JOB_THREADS:-$PHYS_CORES}"
WORKER_THREADS="${HORIZON_WORKER_THREADS:-$PHYS_CORES}"
export HORIZON_JOB_THREADS="$JOB_THREADS"
export HORIZON_WORKER_THREADS="$WORKER_THREADS"
# ----------------------------------------

case "${1:-status}" in
  start)
    "$SERVER_BIN" --dir "$DIR" start
    echo "  服务端已在后台启动。启动横幅见 logs/supervisor.log"
    echo "  登录/注册、区块预生成已内置于服务端，无需外部插件。"
    ;;
  stop)    "$SERVER_BIN" --dir "$DIR" stop ;;
  restart) "$SERVER_BIN" --dir "$DIR" restart ;;
  status)  "$SERVER_BIN" --dir "$DIR" status ;;
  console) "$SERVER_BIN" --dir "$DIR" console ;;
  logs)    "$SERVER_BIN" --dir "$DIR" logs -f -n 50 ;;
  run)     exec "$SERVER_BIN" --dir "$DIR" run ;;
  cmd)     shift; "$SERVER_BIN" --dir "$DIR" cmd "$@" ;;
  # 内置 Chunky 式预生成的快捷入口，转发到服务端 /pregen 命令。
  pregen)
    shift
    if [ "$#" -eq 0 ]; then
      echo "用法: $0 pregen <半径> [square|circle]  (例: $0 pregen 512 circle)"
      echo "      $0 pregen status|pause|continue|cancel"
      exit 1
    fi
    "$SERVER_BIN" --dir "$DIR" cmd pregen "$@"
    ;;
  *)
    echo "用法: $0 {start|stop|restart|status|console|logs|run|cmd|pregen}"
    echo "  cmd <命令>            下发控制台命令（如 cmd list）"
    echo "  pregen <半径> [形状]  预生成区块（内置 Chunky 功能，多线程）"
    echo "  pregen status|pause|continue|cancel  管理预生成任务"
    echo "多线程: HORIZON_JOB_THREADS=16 $0 start  (当前 worker=$JOB_THREADS, cores=$WORKER_THREADS)"
    exit 1
    ;;
esac
