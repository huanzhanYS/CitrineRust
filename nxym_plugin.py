#!/usr/bin/env python3
"""
nxym 服务端插件 (类似 Paper 插件, 基于 Horizon 控制 API)
==========================================================
Horizon 是原生 Rust 服务端, 没有 Bukkit/Spigot 那种插件 API。
本插件通过官方控制 API (/api/v1/events + /api/v1/command) 在外围实现:

  1) 登录/注册系统 (类似 AuthMe):
     在聊天框输入 (不带斜杠) register <密码>  注册账号
     在聊天框输入 login <密码>  登录
     changepassword <旧> <新>  改密码
     未登录玩家被限制: 定时提醒 + 超时踢出 (可通过配置强制白名单准入)

  2) 预加载区块 (类似 Chunky):
     /pregen start <半径> [square|circle]   开始预生成
     /pregen status / pause / continue / cancel

事件驱动: 轮询 GET /api/v1/events?since=N, 解析 chat/join/leave 事件,
再用 POST /api/v1/command 下发命令。不修改服务端本体, 不依赖 Java。

用法:
  python3 nxym_plugin.py start        # 后台启动守护进程
  python3 nxym_plugin.py stop
  python3 nxym_plugin.py status
  python3 nxym_plugin.py users        # 列出已注册账号
  python3 nxym_plugin.py pregen <半径> [square|circle]  # 直接发起预生成
  python3 nxym_plugin.py run          # 前台运行 (调试)
"""
import os
import sys
import json
import time
import hashlib
import signal
import urllib.request
import urllib.error
from pathlib import Path

HERE = Path(__file__).resolve().parent
API_JSON = HERE / ".horizon" / "api.json"
DB_FILE = HERE / "config" / "nxym_users.json"
CFG_FILE = HERE / "config" / "nxym_plugin.json"
PID_FILE = HERE / ".horizon" / "nxym-plugin.pid"
LOG_FILE = HERE / "logs" / "nxym-plugin.log"

DEFAULT_CFG = {
    "auth_enabled": True,
    "kick_unlogged_after_seconds": 90,   # 未登录宽限期, 0 表示不踢
    "remind_every_seconds": 20,          # 未登录提醒间隔
    "case_insensitive_name": True,
    "min_password_length": 4,
}

# ---------------------------------------------------------------- API 客户端
class Api:
    def __init__(self):
        info = json.loads(API_JSON.read_text())
        self.addr = info["address"]
        self.token = info["token"]
        self.base = f"http://{self.addr}/api/v1"

    def _req(self, path, method="GET", body=None):
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(
            self.base + path, data=data, method=method,
            headers={
                "Authorization": f"Bearer {self.token}",
                "Host": self.addr,
                "Content-Type": "application/json",
            },
        )
        with urllib.request.urlopen(req, timeout=8) as r:
            return json.loads(r.read().decode("utf-8", "replace"))

    def events(self, since=0, types=""):
        p = f"/events?since={since}"
        if types:
            p += f"&types={types}"
        return self._req(p).get("events", [])

    def command(self, line):
        return self._req("/command", "POST", {"line": line}).get("output", [])

    def players(self):
        return self._req("/players").get("online", [])

    def status(self):
        return self._req("/status")

# ---------------------------------------------------------------- 账号存储
def load_json(path, default):
    try:
        return json.loads(path.read_text())
    except Exception:
        return default

def save_json(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(data, ensure_ascii=False, indent=2))
    tmp.replace(path)

def norm_name(name, cfg):
    return name.lower() if cfg.get("case_insensitive_name", True) else name

def hash_pw(password, salt=None):
    salt = salt or os.urandom(16).hex()
    digest = hashlib.sha256((salt + password).encode()).hexdigest()
    return f"sha256${salt}${digest}"

def check_pw(password, stored):
    try:
        _, salt, digest = stored.split("$", 2)
        return hashlib.sha256((salt + password).encode()).hexdigest() == digest
    except Exception:
        return False

# ---------------------------------------------------------------- 守护逻辑
def log(msg):
    line = f"[{time.strftime('%Y-%m-%d %H:%M:%S')}] {msg}"
    LOG_FILE.parent.mkdir(parents=True, exist_ok=True)
    with LOG_FILE.open("a") as f:
        f.write(line + "\n")
    print(line, flush=True)

class NxymPlugin:
    def __init__(self):
        self.api = Api()
        self.cfg = load_json(CFG_FILE, DEFAULT_CFG)
        if not CFG_FILE.exists():
            save_json(CFG_FILE, self.cfg)
        self.db = load_json(DB_FILE, {})       # name -> {salt_hash, ip, registered, last_login}
        self.sessions = {}                     # name -> login timestamp (内存态)
        self.pending = {}                      # name -> {"since":t, "reminded":t}
        self.running = True

    @staticmethod
    def _strip_codes(message):
        # § 颜色码在 JSON 文本组件里会原样显示, 这里去掉, 用 color 字段代替
        import re
        return re.sub(r'§[0-9a-fk-or]', '', message)

    def tell(self, player, message):
        # tellraw 给单个玩家, JSON 文本组件; 依据 § 码选择颜色
        color = "gold"
        if "§a" in message:
            color = "green"
        elif "§c" in message:
            color = "red"
        elif "§e" in message:
            color = "yellow"
        payload = json.dumps({"text": self._strip_codes(message), "color": color}, ensure_ascii=False)
        self.api.command(f'tellraw {player} {payload}')

    def broadcast(self, message):
        self.api.command(f'say [nxym] {message}')

    def is_authed(self, name):
        key = norm_name(name, self.cfg)
        return key in self.sessions

    def handle_chat(self, ev):
        name = ev.get("name", "")
        msg = ev.get("message", "").strip()
        if not name or not msg:
            return
        key = norm_name(name, self.cfg)
        parts = msg.split()
        raw_cmd = parts[0].lower() if parts else ""
        # 原版聊天: 玩家输入 "/xxx" 走命令系统, 不产生 chat 事件, 插件收不到。
        # 因此用不带斜杠的裸词作为命令 (会短暂出现在聊天里, 属正常); 同时兼容带斜杠形式。
        cmd = raw_cmd[1:] if raw_cmd.startswith("/") else raw_cmd

        if cmd == "register":
            if key in self.db:
                self.tell(name, "§c你已经有账号了, 请用 login <密码> 登录")
                return
            if len(parts) < 2:
                self.tell(name, "§e用法: register <密码>")
                return
            pw = parts[1]
            if len(pw) < self.cfg.get("min_password_length", 4):
                self.tell(name, f"§c密码至少 {self.cfg['min_password_length']} 位")
                return
            self.db[key] = {"hash": hash_pw(pw), "registered": time.time(), "last_login": 0}
            save_json(DB_FILE, self.db)
            self.sessions[key] = time.time()
            self.pending.pop(key, None)
            self.tell(name, "§a注册成功! 你已自动登录, 欢迎来到 nxym 服务器")
            log(f"register: {name}")

        elif cmd == "login":
            if key not in self.db:
                self.tell(name, "§c你还没有账号, 请先用 register <密码> 注册")
                return
            if len(parts) < 2:
                self.tell(name, "§e用法: login <密码>")
                return
            if check_pw(parts[1], self.db[key]["hash"]):
                self.sessions[key] = time.time()
                self.pending.pop(key, None)
                self.db[key]["last_login"] = time.time()
                save_json(DB_FILE, self.db)
                self.tell(name, "§a登录成功! 祝游戏愉快")
                log(f"login: {name}")
            else:
                self.tell(name, "§c密码错误, 请重试")

        elif cmd == "changepassword":
            if key not in self.sessions:
                self.tell(name, "§c请先 login 登录")
                return
            if len(parts) < 3:
                self.tell(name, "§e用法: changepassword <旧密码> <新密码>")
                return
            if check_pw(parts[1], self.db[key]["hash"]):
                self.db[key]["hash"] = hash_pw(parts[2])
                save_json(DB_FILE, self.db)
                self.tell(name, "§a密码已修改")
            else:
                self.tell(name, "§c旧密码错误")

    def handle_join(self, ev):
        name = ev.get("name", "")
        key = norm_name(name, self.cfg)
        log(f"join event: {name}")
        if key in self.sessions:
            self.tell(name, "§a欢迎回来! 你已登录")
            return
        self.pending[key] = {"since": time.time(), "reminded": 0}
        if key in self.db:
            self.tell(name, "§e这是注册账号! 请输入 login <密码> 登录")
        else:
            self.tell(name, "§e本服需要注册! 请输入 register <密码> 创建账号")

    def handle_leave(self, ev):
        key = norm_name(ev.get("name", ""), self.cfg)
        self.pending.pop(key, None)
        self.sessions.pop(key, None)  # 掉线即登出

    def tick_auth(self):
        if not self.cfg.get("auth_enabled", True):
            return
        online = {norm_name(p.get("name", ""), self.cfg): p for p in self.api.players()}
        # 同步: 已离线但内存还留着的会话清掉
        for key in list(self.sessions):
            if key not in online:
                self.sessions.pop(key, None)
                self.pending.pop(key, None)
        grace = self.cfg.get("kick_unlogged_after_seconds", 90)
        remind = self.cfg.get("remind_every_seconds", 20)
        now = time.time()
        for key, info in list(self.pending.items()):
            if key in self.sessions or key not in online:
                continue
            name = online[key].get("name", key)
            elapsed = now - info["since"]
            if grace > 0 and elapsed >= grace:
                self.api.command(f'kick {name} 未登录: 请先 register 或 login')
                log(f"kick unlogged: {name}")
                self.pending.pop(key, None)
            elif remind > 0 and now - info["reminded"] >= remind:
                if key in self.db:
                    self.tell(name, "§e提醒: 请用 login <密码> 登录, 否则会被踢出")
                else:
                    self.tell(name, "§e提醒: 请用 register <密码> 注册, 否则会被踢出")
                info["reminded"] = now

    def loop(self):
        log("nxym 插件启动, 监听控制 API 事件流...")
        # 从当前最新事件开始, 避免重放历史 join/leave 给老玩家重复发提示
        try:
            latest = self.api.events(0)
            since = max((e.get("id", 0) for e in latest), default=0)
        except Exception:
            since = 0
        log(f"事件游标起点: {since}")
        last_tick = 0
        while self.running:
            try:
                events = self.api.events(since)
                for ev in events:
                    since = max(since, ev.get("id", 0))
                    t = ev.get("type")
                    if t == "chat":
                        self.handle_chat(ev)
                    elif t == "join":
                        self.handle_join(ev)
                    elif t == "leave":
                        self.handle_leave(ev)
                if time.time() - last_tick >= 3:
                    self.tick_auth()
                    last_tick = time.time()
            except Exception as e:
                log(f"轮询异常: {e}")
            time.sleep(0.8)

# ---------------------------------------------------------------- CLI
def ensure_api():
    if not API_JSON.exists():
        print("错误: 找不到 .horizon/api.json, 请先启动服务器 (./start.sh start)")
        sys.exit(1)

def cmd_start(foreground=False):
    ensure_api()
    if PID_FILE.exists() and not foreground:
        pid = int(PID_FILE.read_text())
        try:
            os.kill(pid, 0)
            print(f"插件已在运行 (pid {pid})")
            return
        except ProcessLookupError:
            pass
    if foreground:
        NxymPlugin().loop()
        return
    pid = os.fork()
    if pid > 0:
        PID_FILE.write_text(str(pid))
        print(f"nxym 插件已后台启动 (pid {pid}), 日志: {LOG_FILE}")
        return
    # 子进程: 脱离终端
    os.setsid()
    devnull = os.open(os.devnull, os.O_WRONLY)
    os.dup2(devnull, sys.stdout.fileno())
    os.dup2(devnull, sys.stderr.fileno())
    NxymPlugin().loop()

def cmd_stop():
    if not PID_FILE.exists():
        print("插件未在运行")
        return
    pid = int(PID_FILE.read_text())
    try:
        os.kill(pid, signal.SIGTERM)
        PID_FILE.unlink()
        print(f"已停止插件 (pid {pid})")
    except ProcessLookupError:
        PID_FILE.unlink()
        print("插件进程已不存在, 已清理 pid 文件")

def cmd_status():
    ensure_api()
    api = Api()
    st = api.status()
    phase = st.get("phase")
    game = st.get("sample", {}).get("game", {})
    online = game.get("players", [])
    plugin_running = False
    if PID_FILE.exists():
        try:
            os.kill(int(PID_FILE.read_text()), 0)
            plugin_running = True
        except ProcessLookupError:
            pass
    db = load_json(DB_FILE, {})
    print(f"服务器      : {phase}  (TPS {game.get('tps', 0):.1f}, {len(online)} 人在线)")
    print(f"nxym 插件   : {'运行中' if plugin_running else '未运行'}")
    print(f"注册账号数  : {len(db)}")
    print(f"插件日志    : {LOG_FILE}")

def cmd_users():
    db = load_json(DB_FILE, {})
    if not db:
        print("暂无注册账号")
        return
    print(f"{'账号':<20} {'注册时间':<22} {'最后登录':<22}")
    for name, info in sorted(db.items()):
        reg = time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(info.get("registered", 0)))
        last = time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(info.get("last_login", 0)))
        print(f"{name:<20} {reg:<22} {last:<22}")

def cmd_pregen(radius, shape="square"):
    ensure_api()
    api = Api()
    out = api.command(f"pregen start {radius} {shape}")
    print("\n".join(out) if out else "pregen 命令已下发")
    log(f"pregen start {radius} {shape}")

def main():
    args = sys.argv[1:]
    sub = args[0] if args else "status"
    if sub == "start":
        cmd_start(False)
    elif sub == "run":
        cmd_start(True)
    elif sub == "stop":
        cmd_stop()
    elif sub == "status":
        cmd_status()
    elif sub == "users":
        cmd_users()
    elif sub == "pregen":
        cmd_pregen(args[1] if len(args) > 1 else "512", args[2] if len(args) > 2 else "square")
    elif sub == "register":
        # 控制台直接注册 (给管理员用)
        if len(args) < 3:
            print("用法: nxym_plugin.py register <玩家名> <密码>")
            sys.exit(1)
        db = load_json(DB_FILE, {})
        db[norm_name(args[1], DEFAULT_CFG)] = {"hash": hash_pw(args[2]), "registered": time.time(), "last_login": 0}
        save_json(DB_FILE, db)
        print(f"已为 {args[1]} 注册账号")
    else:
        print(__doc__)
        sys.exit(1)

if __name__ == "__main__":
    main()
