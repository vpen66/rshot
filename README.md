# rshot (RustShot)

用 Rust 编写的跨平台截图工具：冻结屏幕 → 交互选区 → 标注编辑 → 复制 / 保存 / 置顶贴图。基于 `xcap` 采集屏幕，`eframe`/`egui` 渲染交互界面，支持 HiDPI 高分屏。

默认以**菜单栏托盘常驻**运行，通过全局快捷键或托盘菜单触发截图，启动时不会立即截屏。

> 当前版本聚焦**截图**能力。CLI 简介中提到的录屏（screen recording）尚未实现，详见[已知限制](#已知限制与待办)。

## 功能特性

### 截图与选区
- **交互式区域截图**：触发后冻结主显示器画面，全屏覆盖层上拖拽框选
- **选区编辑**：拖动移动、8 个控制点缩放、实时显示选区物理像素尺寸（如 `800 × 600`）
- **像素放大镜**：未选区时跟随光标显示 9×9 像素网格，附 HEX / RGB 颜色值与坐标，便于精确取色和对齐
- **CLI 直拍模式**：`--fullscreen` 跳过交互界面，直接截取主屏并复制到剪贴板 + 保存文件
- **HiDPI 支持**：按显示器缩放因子换算，标注以物理分辨率烘焙导出，Retina 屏不模糊

### 托盘常驻与全局快捷键
- **菜单栏常驻**：默认启动后驻留系统托盘（macOS 菜单栏），不立即截屏；托盘菜单提供「区域截图 / 全屏截图 / 编辑配置 / 重载配置 / 退出」
- **全局快捷键**：默认 `Cmd+Option+A` 区域截图、`Cmd+Option+F` 全屏截图（macOS；其他平台对应 `Ctrl+Alt`）
- **TOML 配置**：快捷键写入配置文件，可在托盘菜单「编辑配置」打开修改，「重载配置」即时生效，无需重启
  - macOS：`~/Library/Application Support/rshot/config.toml`
  - Linux：`~/.config/rshot/config.toml`
  - Windows：`%APPDATA%\rshot\config.toml`
- **子进程截图**：托盘进程通过快捷键 / 菜单拉起独立截图子进程，规避 macOS 单进程仅允许一个事件循环的限制

### 标注工具（10 种）
| 工具 | 说明 |
| --- | --- |
| 选区 | 默认模式，调整选区 |
| 矩形 / 圆形 | 拖拽绘制矩形、椭圆（圆）边框 |
| 箭头 / 直线 | 带箭头的指示线、普通直线 |
| 画笔 | 自由手绘 |
| 高亮 | 半透明荧光笔（3 倍笔宽） |
| 马赛克 | 区域像素化打码（导出时生效，预览为网格占位） |
| 文字 | 单击放置，内联输入框回车确认 |
| 序号 | 单击放置 ①②③ 步骤徽章，自动递增，撤销时回退计数 |

- 9 种预设颜色（红 / 橙 / 黄 / 绿 / 青 / 蓝 / 紫 / 白 / 黑）
- 3 档笔宽（细 2px / 中 4px / 粗 8px）
- 无限撤销 / 重做
- 浮动工具栏自动避让屏幕边缘（选区下方空间不足时翻转到上方）

### 导出
- **复制到剪贴板**：选区 + 全部标注烘焙后的图像
- **保存 PNG**：默认目录 `~/Pictures/RustShot/`，时间戳命名 `rshot_YYYY-MM-DD_HHMMSS.png`
- **置顶贴图（Pin）**：把选区变成悬浮贴纸窗口

### 贴图窗口（Pin）
- 无边框、始终置顶、透明背景，出现在原选区位置
- 任意位置按住拖动移动窗口
- 滚轮缩放（10% – 1000%），`Ctrl/Cmd + 滚轮` 调节透明度（15% – 100%）
- `R` 顺时针旋转 90°，`X` 切换鼠标穿透（点击穿过贴纸操作下层窗口）
- 右键菜单：复制 / 保存 / 旋转 / 恢复 100% / 鼠标穿透 / 关闭
- 双击或 `Esc` 关闭；操作时中央显示渐隐提示徽章

### CLI 辅助
- `--list-monitors`：列出所有显示器（分辨率、位置、缩放因子、主屏标记）
- `--list-windows`：列出当前可见窗口（标题、所属应用、位置尺寸）

## 快捷键

### 全局快捷键（托盘常驻时生效，可在配置文件修改）
| 按键 | 功能 |
| --- | --- |
| `Cmd+Option+A` | 区域截图（拉起交互覆盖层） |
| `Cmd+Option+F` | 全屏截图（复制 + 保存） |

> 非 macOS 平台默认对应 `Ctrl+Alt+A` / `Ctrl+Alt+F`。修改 `config.toml` 后在托盘菜单点「重载配置」生效。

### 截图覆盖层
| 按键 | 功能 |
| --- | --- |
| 拖拽 | 创建 / 移动 / 缩放选区，或绘制当前标注 |
| `Enter` / `Ctrl+C` / 双击选区 | 复制并退出 |
| `Ctrl+S` | 保存并退出 |
| `P` / `F3` | 置顶贴图并退出 |
| `Ctrl+Z` | 撤销 |
| `Ctrl+Shift+Z` / `Ctrl+Y` | 重做 |
| `Esc` | 先退回选区工具，再按退出 |

### 贴图窗口
| 按键 | 功能 |
| --- | --- |
| 滚轮 | 缩放 |
| `Ctrl/Cmd + 滚轮` | 透明度 |
| `R` | 旋转 90° |
| `X` | 鼠标穿透开关 |
| `Ctrl+C` / `Ctrl+S` | 复制 / 保存 |
| 双击 / `Esc` | 关闭 |

## 构建与运行

要求：Rust **1.85+**（edition 2024）。

```bash
# 构建
cargo build --release

# 托盘常驻（默认）：驻留菜单栏，用全局快捷键 / 托盘菜单触发截图
cargo run --release

# 立即拉起交互式区域截图（跳过托盘）
cargo run --release -- --capture

# 直接全屏截图
cargo run --release -- --fullscreen

# 全屏截图并指定输出路径
cargo run --release -- --fullscreen --output ~/Desktop/shot.png

# 查看显示器 / 窗口信息
cargo run --release -- --list-monitors
cargo run --release -- --list-windows

# 运行单元测试
cargo test --workspace
```

日志级别通过 `RUST_LOG` 控制（默认 `info`），例如 `RUST_LOG=debug cargo run`。

### 平台权限
- **macOS**：首次运行需在「系统设置 → 隐私与安全性 → 屏幕录制」中授权终端 / 应用，否则捕获画面为空白
- **Windows / Linux**：依赖 `xcap` 的平台采集能力（Linux 支持 X11，Wayland 下能力受合成器限制）

## 项目架构

Cargo workspace，6 个 crate，依赖方向自上而下：

```
rshot (bin, CLI 入口)
├── rshot-tray      菜单栏托盘常驻 + 全局快捷键 + TOML 配置
├── rshot-overlay   交互截图覆盖层（eframe/egui）
│   ├── rshot-pin   置顶贴图窗口（eframe/egui）
│   ├── rshot-core
│   └── rshot-storage
├── rshot-pin
├── rshot-core
└── rshot-storage
```

| Crate | 职责 | 关键依赖 |
| --- | --- | --- |
| `rshot`（根） | CLI 参数解析、模式分发（托盘常驻 / 直拍 / 交互 / 贴图接力） | clap, anyhow, env_logger |
| `rshot-tray` | 菜单栏托盘常驻、全局快捷键注册、TOML 配置读写与重载；按快捷键 / 菜单拉起截图子进程 | tray-icon, global-hotkey, winit, serde, toml, dirs |
| `rshot-core` | 屏幕 / 窗口采集（`ScreenCapture`）、剪贴板（`ClipboardService`）、图像绘制与特效（`ImageProcessor`：马赛克、模糊、箭头、序号徽章等） | xcap, arboard, image, thiserror |
| `rshot-overlay` | 冻结屏覆盖层：选区状态机、放大镜、标注渲染与烘焙、撤销历史、工具栏 | eframe, egui |
| `rshot-pin` | 贴图窗口：缩放 / 透明度 / 旋转 / 鼠标穿透 / 右键菜单 | eframe, egui |
| `rshot-storage` | 保存目录与时间戳文件名、PNG/JPEG 输出格式、存储配置结构 | chrono, dirs, serde |

数据流：`ScreenCapture` 抓取主屏 → 覆盖层以 GPU 纹理显示冻结画面 → 用户选区与标注（逻辑坐标）→ 导出时按 `scale_factor` 裁剪原图并把标注烘焙到物理像素 → 剪贴板 / 磁盘 / `PinRequest` 传递给贴图窗口。

## 已知限制与待办

按影响程度排序：

1. **文字标注不会真正导出**：覆盖层中可见文字，但烘焙导出时仅绘制一个颜色圆点占位（`rshot-overlay/src/annotation.rs` 的 `Text` 分支），缺少文字光栅化。这是当前最主要的功能缺口
2. **仅支持主显示器交互截图**：`capture_monitor_by_index` 已实现但未接入 UI；多显示器拼接覆盖层未实现
3. **窗口截图未接入**：`--list-windows` 与窗口枚举已就绪，但尚无「点击窗口自动吸附截取」功能
4. **录屏未实现**：CLI 简介宣称的 screen recording 无任何对应代码
5. **存储配置未持久化**：快捷键已写入 `config.toml` 并支持重载，但 `StorageConfig`（保存目录 / 格式 / 前缀）仍未读写配置文件；界面保存固定为 PNG，JPEG 格式仅在库层支持；`--output` 只对 `--fullscreen` 模式生效
6. **马赛克预览为占位网格**：真实像素化效果仅在导出后呈现
7. **模糊特效未暴露**：`ImageProcessor::apply_blur` 已实现并有测试，但工具栏没有对应工具
8. **文字输入为单行**：不支持多行文本标注

## 测试

单元测试覆盖：选区裁剪边界、马赛克 / 模糊 / 图元绘制、撤销重做与序号计数、选区归一化与控制点命中、贴图旋转与默认状态、文件名生成与落盘。

```bash
cargo test --workspace
```