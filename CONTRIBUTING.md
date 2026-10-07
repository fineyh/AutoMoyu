# 参与开发

欢迎提 Issue 和 PR。改动比较大（新功能、改识别算法）的话，建议先开个 Issue 聊一下思路，免得白写。

## 环境

- Windows 10 2004+ / Windows 11（平台层只支持 Windows）
- [Rust](https://rustup.rs/) stable，以及 MSVC 生成工具（Visual Studio Build Tools，勾选「使用 C++ 的桌面开发」）
- Node.js 22
- pnpm：版本写在根目录 `package.json` 的 `packageManager` 里，执行 `corepack enable` 后会自动使用对应版本

## 常用命令

```bash
pnpm install
pnpm dev                 # 启动桌面应用，前端热重载
pnpm web                 # 只跑界面，用假数据，浏览器打开 http://localhost:1420/
cargo test --workspace   # 全部 Rust 测试
pnpm typecheck           # 前端类型检查
pnpm build               # 打安装包，输出在 target/release/bundle/nsis/
```

`pnpm web` 可以用 URL 参数切换场景，不开游戏也能调界面：

```text
?mock=running | paused | takeover | idle | calib | done | nowin | update
&lang=en | zh
```

## 目录结构

```text
crates/moyu-core    识别算法和状态机，不依赖任何平台 API，可以直接单测
crates/moyu-win     Windows 平台层：找窗口、截图、SendInput、进程声音环回、键鼠钩子
crates/moyu-bench   命令行工具：探测、录制真实片段、离线评估
src-tauri/          桌面应用：服务线程、校准流程、统计库（SQLite）、托盘、浮层、热键、自动更新
app/                界面（React + TypeScript + Tailwind）
tools/fakegame/     假的 "Minecraft" 窗口和端到端冒烟脚本
legacy/python/      v0.1 的 Python 原型，只作参考，不再维护
```

识别相关的逻辑尽量放在 `moyu-core`，保持没有 I/O，这样才能用录下来的数据反复回放测试。

## 用真实录像评估识别效果

改识别算法前后，最好用真实游戏录像对比一下。`fixtures/` 下的录像体积大，也包含个人游戏画面，所以不放进仓库，需要自己录：

```bash
cargo run -p moyu-bench -- probe       # 确认能找到游戏窗口、截到画面、录到声音
cargo run -p moyu-bench --release -- record fixtures/day-machine --scenario machine
cargo run -p moyu-bench --release -- analyze fixtures/day-machine
```

录制时正常钓鱼，程序只看不点：

- 有钓鱼机用 `--scenario machine`，你的每次右键都算甩竿
- 没有钓鱼机用 `--scenario manual`，你的右键依次算作甩竿、收竿

`analyze` 用前两轮做校准，其余全部当测试，输出鱼竿状态的准确率、两种状态的区分度，以及咬钩声音的召回率和误报数。设置环境变量 `MOYU_VERBOSE=1` 会列出判错的帧。

`dusk` 子命令把白天和夜晚两段录像做淡入淡出，用来测昼夜交替：

```bash
cargo run -p moyu-bench --release -- dusk fixtures/day-machine fixtures/night-machine
```

## 端到端冒烟

没有游戏也能跑通整条链路：脚本会起一个假游戏窗口和 debug 版 AutoMoyu，自动完成校准并钓 25 秒，最后核对统计条数。

```bash
pnpm web                         # 另开一个终端，debug 版从开发服务器加载界面
cargo build -p automoyu
python tools/fakegame/e2e.py
```

脚本会抢焦点、移动鼠标、按 F6，跑的时候别用电脑。它会先备份再清空 `%APPDATA%\AutoMoyu` 下的设置和校准。

## 提交 PR 之前

- `cargo test --workspace` 和 `pnpm typecheck` 都能通过（CI 会跑同样的检查）
- 界面文字中英文都要写，用 `t("中文", "English")`
- 改了识别算法的，在 PR 里贴一下 `analyze` 前后的结果
- 提交信息沿用现有格式：`feat: ...`、`fix: ...`、`chore: ...`，英文小写开头
- 用户能感知到的改动，在 [CHANGELOG.md](CHANGELOG.md) 的「未发布」下面加一行

## 发布（维护者）

1. 把 `CHANGELOG.md` 的「未发布」改成新版本号和日期
2. 同步修改 `Cargo.toml`、`src-tauri/tauri.conf.json`、`app/package.json` 里的版本号
3. 提交后打 tag 并推送：`git tag v1.2.3 && git push origin v1.2.3`

推送 `v*` tag 会触发 GitHub Actions：构建 NSIS 安装包，用 minisign 私钥给更新包签名，生成 `latest.json`，并把 CHANGELOG 里这个版本的内容作为发布说明。

版本号带 `-` 的（比如 `v1.0.0-beta.2`）会标成预发布，只推送给选了「测试版」更新通道的用户；正式版两个通道都会收到。

仓库需要配置 Secret `TAURI_SIGNING_PRIVATE_KEY`；私钥设了密码的话，再配 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。签名私钥一旦丢失，已安装的客户端就无法再收到更新。
