# AutoMoyu 🎣 — Minecraft 基岩版自动钓鱼

**一次选择，一次校准，然后去摸鱼。**

你只回答一个问题——有没有钓鱼机；程序自己甩两次竿、挑出最好认的画面、定好阈值，然后按 F6 就能挂机。
没甩出去会自己重甩，切出游戏会自动暂停、切回来自动继续，停下来一定告诉你原因和怎么办。

| 模式 | 适用 | 程序做什么 |
|---|---|---|
| **自动甩竿**（主推） | 有半自动钓鱼机：上钩后机器自动收竿 | 发现"竿收回来了"就再甩一次 |
| **全自动**（测试版） | 没有钓鱼机，对着水面 | 听到咬钩的水花声就收竿，收回来再甩 |

## 安装

到 [Releases](https://github.com/fineyh/AutoMoyu/releases) 下载 `AutoMoyu_x.y.z_x64-setup.exe`，双击安装（不需要管理员权限）。
安装包还没做代码签名，Windows 可能提示"未知发布者"：点 **更多信息 → 仍要运行**。

之后会自动检查更新；钓鱼进行中不会打扰，停下来后首页会提示"重启并更新"。

## 使用

1. 打开 Minecraft 基岩版，改成**窗口化或无边框**（独占全屏截不到画面）。
2. 手持鱼竿，站在钓鱼机的位置（全自动：准星对着水面）。
3. 第一次打开 AutoMoyu 会进入校准：切回游戏按 **F6**，程序自己甩两次竿，约 10 秒。
4. 校准完成后再按 **F6** 开始钓鱼；再按一次暂停。**F7** 显示/隐藏游戏内浮层。

窗口尺寸变了会提示重新校准（每个尺寸只需校准一次）。

**小建议**：关掉游戏里的"视角摇晃"、别开光影，信号会更稳；全自动模式下"音效"音量别调到 0（音乐可以关）。

## 常见问题

- **找不到 Minecraft**：确认游戏是窗口化/无边框；还不行就在 设置 → 高级 → 游戏窗口 里手动选。
- **没甩出去**：手上可能不是鱼竿，或面前没有水。修好后按 F6 继续。
- **认不出鱼竿**：通常是打开了菜单/背包，关掉就会自动继续；一直不行就重新校准。
- **反馈问题**：设置 → 高级 → 导出诊断包，把 zip 附到 [Issue](https://github.com/fineyh/AutoMoyu/issues) 里。
- **旧版数据**：统计 → 导入旧版数据，选 v0.1 的 `data/stats.json`。

> AutoMoyu 只在游戏窗口里模拟鼠标右键，不读写游戏内存、不修改游戏文件。多人服务器请遵守服务器规则。

---

## 开发

技术栈：Tauri 2 + Rust + React 19 / TypeScript / Tailwind v4。需要 Rust stable、Node 22、pnpm。

```text
crates/moyu-core   纯算法（无平台依赖，可单测）：竿状态两模板分类、自动校准选区、电平触发状态机、声音咬钩
crates/moyu-win    Windows 平台层：找窗口、GDI 截图、SendInput、防睡眠、WASAPI 进程环回、鼠标钩子
crates/moyu-bench  命令行：probe 冒烟 / record 录真实片段 / analyze 离线评估
src-tauri/         桌面应用：服务线程、校准流程、统计库(SQLite)、托盘、浮层、热键、自动更新
app/               界面
tools/fakegame/    假 "Minecraft" 窗口 + 端到端冒烟脚本（没有游戏也能跑通整条链路）
legacy/python/     v0.1 原型（tag v0.1.0），功能对齐后删除
```

```bash
pnpm install
pnpm dev                 # 启动应用（热重载）
pnpm web                 # 只看界面：浏览器打开 http://localhost:1420/?mock=running|paused|idle|calib|done|nowin
cargo test --workspace   # 算法、平台冒烟、应用单测
```

### 用真实录像评估算法（Phase 0）

```bash
cargo run -p moyu-bench -- probe                                   # 找窗口、截图、听 2 秒声音
cargo run -p moyu-bench --release -- record fixtures/day-machine --scenario machine
cargo run -p moyu-bench --release -- analyze fixtures/day-machine
```

录制时正常钓鱼即可（程序只看不点）：有钓鱼机用 `--scenario machine`，没有就 `--scenario manual`（你的右键交替为甩竿/收竿）。
`analyze` 只用前两轮做校准、其余全部当测试，输出竿状态准确率、区分度，以及声音咬钩的召回率和误报。

### 端到端冒烟（假游戏）

```bash
pnpm web &                                  # debug 版加载开发服务器
cargo build -p automoyu
python tools/fakegame/e2e.py                # 会抢焦点、移动鼠标、按 F6，别在用电脑时跑
```

### 发布

推 `v*` tag 触发 GitHub Actions：构建 NSIS 安装包、用 minisign 私钥给更新包签名、生成 `latest.json` 并发布。
带 `-` 的版本号（如 `v1.0.0-beta.2`）自动标为预发布，只推给"测试版"通道。
仓库需要配置 Secrets `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。**私钥丢了就再也没法给老用户推更新，务必离线备份。**
