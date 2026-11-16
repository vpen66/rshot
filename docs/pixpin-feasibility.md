# PixPin 复刻可行性调研（截屏 / 录屏 / 工具栏 / 快捷键 / 设置）

- 调研对象：`/Applications/PixPin.app` 3.6.6.0（`com.depthpicture.PixPin`，TeamID `N6RUM69A6S`）
- 调研日期：2026-11-13
- 调研方法：包结构清点、Mach-O 符号表（`nm -gU -C`）、`otool -L` 依赖图、`strings` 常量提取、用户态配置文件与运行日志取证
- 结论先行：**可以复刻**，这五个模块没有一个是"系统黑魔法"；难的不是能不能，而是**录屏链路的工程量**和**设置/工具栏的数据驱动抽象**

---

## 1. 结论摘要

| 模块 | 复刻难度 | 判断依据 | 预估（单人全职） |
| --- | --- | --- | --- |
| 设置 | ⭐ 低 | 纯 JSON + 数据驱动表单，无系统能力 | 1 周 |
| 快捷键 | ⭐ 低 | Carbon `RegisterEventHotKey` 一套现成方案（`global-hotkey` crate 即可） | 1 周 |
| 工具栏 | ⭐⭐ 中低 | 自研 `ActionsBar` 浮动条框架，逻辑清晰、可自建 | 1.5–2 周 |
| 截屏 | ⭐⭐⭐ 中 | 采集 + 冻结层 + 选区 + 标注重度交互；rshot 已有 60% 基础 | 3–4 周（对齐核心） |
| 录屏 | ⭐⭐⭐⭐ 高 | ScreenCaptureKit 采集 + VideoToolbox/FFmpeg 编码 + 多音轨 + 回放叠加 + 崩溃恢复 | 5–8 周 |

**合计约 3–4 个月**能做出"日常可用、不崩、不掉帧"的对标版本（不含 OCR / 翻译 / 长截图 / 表格识别 / LaTeX / 账号授权 / 云同步）。

**不建议复刻的部分**（不在本次范围，但要明确排除）：OCR（ONNX Runtime + OpenCV + 55MB 模型）、表格识别、翻译、LaTeX、长截图拼接、Airdrop/打印、账号与付费、崩溃上报（Crashpad/Sentry）、自动更新。

---

## 2. PixPin 是什么（实测，非猜测）

**技术栈：Qt 5 Widgets + C++，macOS 14.4 SDK，Universal Binary（x86_64 + arm64），LSUIElement=1（纯菜单栏/托盘应用，无主窗口）。**

证据：

```
$ file MacOS/PixPin
Mach-O universal binary with 2 architectures: [x86_64] [arm64]
$ ls Frameworks/QtCore.framework/Versions/
5                      ← Qt 5，非 Qt 6
$ PlistBuddy Info.plist → LSUIElement = 1, DTSDKName = macosx14.4, CFBundleVersion = 9090
```

**包体构成（这是复刻时的模块划分参照）：**

- 主二进制 `MacOS/PixPin`：18.6 MB，9564 个导出符号 —— **截图编辑器、贴图窗口、设置 UI、动作系统全在这里**
- `Frameworks/`：39 个自研/三方 dylib + 15 个 Qt 框架
- `PlugIns/`：Qt 平台插件（`libqcocoa`）、样式（`libqmacstyle`）、图片格式（gif/icns/ico/jpeg/heic/jp2/tga/tiff/webp）、音视频、SQL、打印
- `Resources/`：`language/*.qm`（9 种语言）、`model/*.onnx|caffemodel`（OCR/超分）、内嵌 icon font

**关键自研模块（按本次范围相关度排序）：**

| dylib | 体量 | 职责（由符号表反推） |
| --- | --- | --- |
| `libPixScreenManager` | 272 KB | `PixScreenManager`（多屏枚举/refresh/wholeRect/screenAt）、`PixScreenStreamer`（`createScreenStreamer`/`captureTo`/`hasFrameChanged`/`updateArea`）、`PixCursorPixmap`（把鼠标指针画进截图） |
| `libPixMovie` | 1.34 MB | 录屏 UI 与编排：`PixMovieUI`、`ScreenRecordWindow` 配套控件、`PixMovieMultiTrackTest`（多轨录制） |
| `libPixAVCodec` | 18.2 MB | **FFmpeg 7.1.1 静态链接**（vcpkg 构建，路径泄漏：`/Users/huangqijun/code/vcpkg/buildtrees/ffmpeg/src/n7.1.1-...`）+ OpenH264 + `PixAVCodecEncoder/Decoder` + `PixMovieEncoderGIF` |
| `libPixActionsBar` | 311 KB | 通用浮动工具栏框架 `ActionsBar` + `ActionsBarLayer` + `FloatPanel` 体系 |
| `libPixConfiguration` | 831 KB | 配置读写 + 设置页工厂 + 安全存储 |
| `libqxtglobalshortcut` | 198 KB | Qxt 全局热键 + 游戏模式/忽略名单增强 |
| `libPixWidget2` | 12 MB | 主要是 Markdown 编辑器（不在本次范围） |

**系统框架依赖（决定了采集与编码路径）：**

```
PixPin            : AVFoundation CoreAudio CoreMedia ScreenCaptureKit
libPixMovie       : AVFoundation AudioToolbox CoreAudio CoreMedia ScreenCaptureKit
libPixAVCodec     : CoreMedia VideoToolbox
libPixScreenManager: CoreGraphics / AppKit / Carbon（静态截图走 CGDisplayCreateImageForRect）
```

即：**静态截屏用 CoreGraphics，录屏用 ScreenCaptureKit + VideoToolbox（硬编），FFmpeg 兜底/封装。**

---

## 3. 五大模块逐项拆解

### 3.1 截屏

**调用链（来自程序自身日志，非推测）：**

```
[PixPin::screenShot] Screenshot response started globalHotkey=true quickShot=false afterSelectAction=N
[PixPin::screenShot] Stage reached stage=prechecksComplete
[PinWindowMacUtils::CheckPermissionWithPrompt] Screen permission status= 3 promptAllowed= true
[PixPin::initScreenShotWindow] Screenshot background prepared
[PixPin::initScreenShotWindow] Screenshot window constructed
[PixPin::screenShot] Stage reached stage=beforeCaptureComplete
[PixPin::screenShot] Screenshot response finished success=true detail=screenshot window displayed
```

要点：
1. **先检查屏幕录制权限**，未授权时走 `CGRequestScreenCaptureAccess` 弹窗，拒绝则 `detail=Screen capture permission denied`
2. **阶段化状态机**（prechecks → 截背景 → 建窗 → 显示），这是防"闪一下黑屏"和失败可恢复的关键设计
3. **多屏模型**独立成 `PixScreenManager`，日志里 `wholeRect= QRect(0,0 1920x1080)`，同时有 `AuxiliaryShotWindow`（副屏辅助窗）、`MacCrossScreenImageWindow`（跨屏选区）、`MacCrossScreenDragPreview`
4. **选区窗口** `ScreenShotWindow` / `ScreenShotView` / `ScreenShotEdit`，副能力：`PresetShotRect(s)` 预设选区、`Screenshot.SelectMode`/`FreeSelectMode`、圆角选区（`Screenshot.enableRoundRect`/`roundRectRatio`）、尺寸显示（`Screenshot.SizeDisplayItems`/`SizeUnit`）
5. **编辑器对象模型**（这是真正的工作量所在）：

   `EditItem*` 家族：`Rect`(几何) / `Line` / `Arrow` / `BrokenLine`(折线) / `Pencil` / `HighLight` / `Mosaic` / `MosaicCustom` / `Blur` / `AutoMosaic`(自动打码) / `RectMosaic` / `RectAutoMosaic` / `Eraser` / `Text` / `TextOutLine` / `Serial`(序号) / `Magnifier`(放大镜) / `Watermark`(水印) / `IconLabel` / `CustomShape` / `Column`
   
   配套：`UndoRedoManger`、`TrigerGraphicEditManage`、`SerialGlobalCounter`（序号全局递增）、`MarkConfigBase`/`MarkSetting`（**每个工具记住上次样式**）

6. **每个工具的默认样式都持久化**，键名如 `MarkBar.Text.Size` / `MarkBar.Arrow.EndArrowType` / `MarkBar.Mosaic.MosaicMode` / `MarkBar.HighLight.Opacity` / `MarkBar.Magnifier.MagnificationValue` —— 共 **约 60 个样式键**，这是"专业感"的主要来源，比工具数量本身更重要

7. **贴图（Pin）**：`PinWindowPixmap` / `PinGraphicsDynamicItem`（支持动态图/视频贴图）/ `PinWindowRoiMap`（贴图缩放后按 ROI 重采样）/ `PinWindowRoute`（历史记录、恢复最近关闭）

### 3.2 录屏

**这是最难的一块，也是 PixPin 与普通截图工具最大的差距。**

链路拆解：

| 环节 | 实现 | 证据 |
| --- | --- | --- |
| 采集 | ScreenCaptureKit（`SCStream`） | `libPixMovie` 链接 `ScreenCaptureKit`；日志 `[PixSystemAudioRecord::stopRecording] ========== Completed (SCK) ==========` |
| 系统音频 | SCK audio 流，独立线程 `PixSystemAudioRecord` | 同上，日志有 `d->captureThread pointer:` |
| 麦克风 | CoreAudio / AudioToolbox | `libPixMovie` 链接 `CoreAudio` `AudioToolbox` |
| 编码 | VideoToolbox 硬编 + FFmpeg 软编兜底 + OpenH264 | `libPixAVCodec` 链接 `VideoToolbox`；内含 `h264Encoder`/`pixGifEncoder`/`webpEncoder`/AAC |
| 容器 | FFmpeg muxer | `libPixAVCodec` 内含完整 libavformat |
| 多轨 | 屏幕轨 + 摄像头轨 + 音频轨 | `PixMovieUI::createMultiTrackRecorder`、`PixMovieMultiTrackTest::testMultiTrackRecording` |
| 崩溃恢复 | `RecordingRecoveryManager` | 主二进制符号 |
| 硬编策略 | `RecordingHardwareAccelerationPolicy` | 主二进制符号 |

**录制参数（配置键即需求清单）：**

```
PixMovie.record.fps                       帧率
PixMovie.record.delay                     开始前延迟
PixMovie.record.enableHardwareAcceleration 硬编开关
PixMovie.record.recordMicrophone           麦克风
PixMovie.record.recordSystemAudio          系统声音
PixMovie.record.mouse / .keyboard          是否记录光标 / 按键
PixMovie.record.camera.enabled/.deviceIndex/.size/.shape/.mirrored/.rotation/.borderColor/.borderWidth
PixMovie.record.recordToolbarDisplayType   录制条展示形态
PixMovie.record.dontShowRecordingWarning   录制提示
```

**"回放叠加"是 PixPin 录屏的差异化能力**（把光标/按键/进度条烧进视频）：

```
PixMovie.playback.mouse.enable / .color / .size / .animationSpeed / .smoothTrack
PixMovie.playback.mouse.simplifyAnimation / .showActionButton / .showWheel
PixMovie.playback.keyboard.keepTime / .maxHistoryCount / .maxContinueChar
PixMovie.playback.progress.enable / .displayMode / .size / .round / .gapTime
PixMovie.playback.progress.bgColor / .fgColor / .progressBarBgColor / .progressBarFgColor
```

**录制工具条控件（由运行日志的 `class=` 字段直接观测）：**

`ActionsBar` 窗口内挂载：
`PixRecordButton`(objectName=`PlayPauseButton`) · `PixRecordTimeLable`(计时) · `PixRecordFpsLabel`(帧率) · `EncodeFormatLabel`(编码格式) · `AudioCtrlWidget` · `RecordAudioCtrlWidget` · `CameraCtrlWidget` · `CursorCtrlWidget` · `KeyboardCtrlWidget` · `KeyboardMouseCtrlLabel` · `ProgressCtrlWidget` · `PlaySpeedLabel` · `ToolBall`

**录制后的播放/裁剪窗口**：`ScreenPlaybackWindow` + `ScreenRecordWindow` + `PixMovieUI::createTrimmedAudioFile`（按轨裁剪音频）。

### 3.3 工具栏

PixPin 的"工具栏"其实是**一个通用浮动条框架**，截图条、录制条、贴图条全部复用：

```
ActionsBar          addButton / addAdvancedButton / addLabel / addIconLabel
                    addGap / addGapLine / addVerticalWidget / addLayerToLayout
                    setArrangeType / setAdvancedDisplayType / setIconSizeScale / resetPixelRatio
                    readjustPos / readjustSize / setPosMode / setPosGlobal / geometryGlobal
                    updateLimitRefRectGlobal / updatePrimaryRefRectGlobal / updateSecondaryRefRectGlobal
                    refreshStyleSheet / addQssFile / sigWheelEventWithCtrl / hasHideReason
ActionsBarLayer     addWidget / addVerticalWidget / setAdvancedButton / setShadowSize
                    updateShadowCache / reAdjustSize / setMinHeight / paintEvent
MoveControl / MoveMarker / ActionsGapLine / AdvancedLabel
```

设计要点（值得抄的架构，不是抄代码）：
1. **Layer 化**：工具栏分主层/展开层，`AdvancedLabel` 是"更多"箭头
2. **参照矩形避让**：`limitRefRect` / `primaryRefRect` / `secondaryRefRect` 三个参照 + `readjustSize`/`readjustPos`，解决"选区贴屏幕边缘时工具条翻到上方"的问题
3. **阴影缓存**：`updateShadowCache`，避免圆角阴影每帧重绘
4. **像素比适配**：`resetPixelRatio`，Retina 上图标不糊
5. **每个按钮可挂浮层面板**：`FloatPanelPenWidthCtrl` / `FloatPanelTextBackground` / `FloatPanelTextOutline` / `FloatPanelShadow` / `FloatPanelRoundRadius` / `FloatPanelHighLightStroke` / `FloatPanelWatermarkParams` / `DotLabelWidthFloatPanel`（宽度指示气泡，日志里出现 15 次 `ButtonWithFloatPanel`）

**工具栏按钮可用户配置**（逐个开关 + 排序）：

```
ScreenShot.ActBarFlag.Copy / Save / QuickSave / Pin / Close / ImageEdit / Print / AirDrop
                          / GifShot / LongShot / OcrDocument / OcrTable / CopyOcr
                          / Translate / LatexRecognition
```

配置入口：`EditButtonsDialog` / `EditActionsDialog` / `EditActionDialog` / `EditToolBallActionsDialog` / `EditPaletteDialog`。

**桌面悬浮球 `ToolBall`**（PixPin 的第二个入口）：`System.DesktopToolBar=2`、`PixDesktopBar.ToolbarPosX/Y`、`ToolBallMenu` / `ToolBallMorePanel` / `ToolBallDropZone` / `ToolBallMacBehavior`，隐藏规则 `DesktopToolBall.AlwaysHide/AlwaysShow/HideOnFullscreen/HideOnScreenshot/HideProcessList`。

### 3.4 快捷键

两层：**通用热键层** + **动作模型层**。

**热键层**：`libqxtglobalshortcut` = Qxt 库（macOS 上落在 Carbon `RegisterEventHotKey`）+ 自研增强 `GlobalShortcutAppClass`：
- `registerShortcut` / `unRegisterShortcut` / `isSetFailed`（注册失败要能告诉用户"这个键被别的应用占了"）
- `checkGameModeStatus` + `setGameModeDisableShortcut`（游戏模式自动禁用）
- `globalIgnoreProgramList` + `onCurrentProcessChanged`（当前前台程序在黑名单里就放行，不抢键）
- `setPrivateEnabled` / `refreshState` / `setShortcutAgain`（改键后重注册）

**动作模型层**（这是精华）：快捷键不直接绑函数，而是绑**动作对象 + 脚本字符串**：

```json
"Action.Screenshot#s.mac": {
  "t": 1794551844,
  "v": {
    "index": 0,
    "isSystemAction": true,
    "script": "pixpin.screenShotAndEdit()",
    "shortCut": "Ctrl+Shift+A",
    "showOnTray": true,
    "title": "Screenshot",
    "type": 256
  }
}
```

注意几个设计：
- 键名带平台后缀 `#s.mac`，跨平台配置分开
- `shortCut` 用 `"Ctrl+Shift+A"` 统一字符串（**macOS 上也叫 Ctrl，不是 Cmd**）——内部再做 `Ctrl↔⌘` 映射
- `showOnTray` 决定是否出现在菜单栏
- `script` 是内嵌脚本 API，共 **33 个函数**：

```
pixpin.screenShot / screenShotAndEdit / delayScreenShot / directScreenShot
pixpin.directScreenShotSpRect / gifScreenShot / gifShotPause / longScreenShot
pixpin.openCustomScreenShot / openConfigurationWindow / switchScreenshotMode
pixpin.pinFromClipBoard / pinFromFiles / pinSelectedFile / closeAllPin
pixpin.hideOrShowAllPin / hideOrShowPinsExceptSelected / restoreLastClosedPin / switchPinGroup
pixpin.setScreenShotRect / getSpRect / genRectUnderCursor / moveCursorBy
pixpin.translateClipboard / trigMousePenetration / toggleWindowTopmostUnderMouse
pixpin.runSystem / moveCursorBy / disableShortcuts / isDisableShortcuts / log
```

- **触发串行化**：日志显示 `PixPinAction::onShortcutTriggered` → `processPendingShortcutTriggers`（`pendingCount=N`），即连按不并发执行，排队处理 —— 这是稳定性的关键细节
- 附加：`System.AllowSimpleHotkey`（允许单键热键）、`KeyPassMenu`（按键穿透给下层窗口）、`BuiltInShortcutManage` + `ShortcutTipsWidget`（编辑器内快捷键提示浮层）

### 3.5 设置

**存储层**：JSON 单文件 `~/Library/Application Support/PixPin/Config/PixPinConfig.json`，每项包一层时间戳：

```json
{ "Save.AutoSave": { "t": 1794552097, "v": "/Users/vpen/Documents/pinpix/PixPin_$yyyy-MM-dd_HH-mm-ss$.png" } }
```

- `t` 是写入时间戳，用途是**多设备/多进程冲突时按时间取新**
- `v` 值类型自由（bool / int / string / object）
- 键是**点分命名空间**，天然支持按前缀分组、导出子集
- 路径模板支持变量：`$yyyy-MM-dd_HH-mm-ss$`
- 敏感数据另存 `LoginData.bin2`（`PixStorage`，带区域标识 `region=cn`、`storageSuffix`，非明文）

**设置页（17 个，实测类名）：**

```
ConfWidgetAbout / Account / Actions / AutoSave / BuiltInShortcut / ColorPalette
/ ConfigManage / FallbackLanguage / ManualSave / NetworkProxy / PinDesktopDisplay
/ QuickRecording / QuickSave / SaveCommon / SerialOutlineColor / TranslateKey
/ TrayConfig / Update
```

加四个插件式控件：`ConfWidgetIntSlider` / `ConfWidgetHardwareAcceleration` / `ConfWidgetClearGlobalMouse` / `ConfWidgetGlobalMouseIgnoreProcessList`。

**关键架构：设置页不是手写页面，而是 `ConfItem` + `createConfWidget` 工厂**：

```
PdConf::ConfWidgetQuickRecording::createConfWidget(PdConf::ConfItem*, QWidget*)
                         ::initUi() / ::loadConf() / ::retranslateUi()
```

即：一个配置项 = {key, 类型, 标题, 默认值, 控件工厂}，页面自动生成，改键只需加一行数据。**这是本项目最值得照搬的设计（模式，不是代码）**。

**配置键规模**：`Save.*` / `System.*` / `Action.*` / `ScreenShot.*` / `Screenshot.*` / `MarkBar.*`(~60) / `Mark.*` / `PixMovie.*`(~40) / `Recording.*` / `Pin.*` / `Ocr.*` / `DesktopToolBall.*` / `PixDesktopBar.*` / `LongShot.*` / `SaveDialog.*` —— 粗估 **200+ 键**。

---

## 4. 与 rshot 现状的差距

rshot 现状：**4550 行 Rust**，6 个 crate（`core` / `overlay` / `pin` / `storage` / `tray` + bin），`xcap` 采集 + `eframe/egui` 渲染 + `global-hotkey` + `tray-icon` + TOML 配置。

| 能力 | rshot 现状 | 对齐 PixPin 的缺口 | 优先级 |
| --- | --- | --- | --- |
| 静态截屏 | ✅ xcap + 冻结层 + 选区 + 8 控制点 | 多显示器拼接、窗口/控件吸附（PixPin 有 `UiRegionDetector` + OpenCV） | P1 |
| 标注 | ⚠️ 10 种工具 | **文字导出是空壳（只画圆点）**；缺放大镜、水印、橡皮、折线、自动打码；样式不记忆 | P0（bug） |
| 放大镜 | ✅ 9×9 取色 | PixPin 是"放大镜标注"（可链接到源区域的圆形放大图），两回事 | P2 |
| 贴图 Pin | ✅ 缩放/旋转/穿透/透明度 | 缺历史记录、动态图/视频贴图、ROI 重采样 | P2 |
| 工具栏 | ⚠️ `toolbar.rs` 553 行，固定布局 | 缺 layer 折叠、浮层面板、按钮可配置、避让策略粗糙 | P1 |
| 快捷键 | ⚠️ 2 个硬编码 | 缺动作表、用户自定义、脚本/命令绑定、注册失败回显、按键穿透 | P1 |
| 设置 | ⚠️ 只有 `config.toml` 存热键 | 缺统一配置存储（`StorageConfig` 未持久化）、设置窗口、热重载、版本迁移 | P1 |
| 录屏 | ❌ 完全没有（README 自认） | 采集 + 编码 + 音轨 + 工具条 + 回放叠加 + 崩溃恢复，**全新 crate** | P0（大头） |

> 注：`文字标注不导出`、`仅主显示器`、`录屏未实现`、`StorageConfig 未持久化` 四条来自仓库 README 自述，属于已知缺口。

---

## 5. 技术选型建议（Rust / macOS 优先）

| 能力 | 推荐方案 | 备选 | 风险 |
| --- | --- | --- | --- |
| 静态截屏 | 继续 `xcap`，补 `objc2-core-graphics` 做 `CGDisplayCreateImageForRect` 精细控制 | `screencapturekit` 单帧 | `xcap` 在新 macOS 上走已废弃 API，长期需迁 SCK |
| 录屏采集 | `screencapturekit` crate（封装 SCStream，含音频） | `objc2-screen-capture-kit` 手写绑定 | SCK 返回 `CMSampleBuffer`，Rust 侧要接 `IOSurface`/`CVPixelBuffer` |
| 视频编码 | `objc2-video-toolbox` 硬编 H.264/HEVC（≤1 帧延迟） | `ffmpeg-next` 软编兜底；`ffmpeg-sidecar` 最省事但多进程 | VT 回调线程模型与 egui 主线程隔离 |
| 音频 | SCK 系统音 + `cpal`（CoreAudio）麦克风，`rubato` 做重采样混音 | AVAudioEngine 绑定 | 双路音轨时钟对齐 |
| 容器 | `mp4` crate 或 ffmpeg muxer | 自写 mp4 box | 需处理关键帧索引/中断恢复 |
| 热键 | `global-hotkey`（Carbon，已够用）+ 自建注册失败检测/游戏模式 | 自写 `RegisterEventHotKey` 绑定 | `global-hotkey` 不告诉你键是否被占用 |
| 设置 | `serde` + JSON（键名对齐 PixPin 语义，但**自己命名**）+ `notify` 热重载 | TOML（现状） | 版本迁移策略要提前定 |
| 设置 UI | 数据驱动表单：`ConfItem { key, kind, default, validator }` → 渲染器 | 手写每页（不可维护） | egui 的表单控件较简陋，需自绘 |
| 工具栏 | 泛化现有 `toolbar.rs`：Layer + 按钮描述表 + 参照矩形避让 | 引入新 UI 框架 | egui 无原生"阴影缓存"，需自建 |
| 崩溃恢复 | 录屏时持续写临时分段，异常退出后扫描重建 | - | 分段 mp4 拼接 |

---

## 6. 复刻的法律与工程边界

**可以做的：**
- 观察行为、拆解架构、学习模块划分与交互流程
- 用公开 API（CoreGraphics / ScreenCaptureKit / VideoToolbox / Carbon）自行实现
- 参考配置键的**语义**（"要有帧率设置"），自己设计命名

**不能做的：**
- 复制二进制、反编译代码直接搬运（Qt C++ → Rust 本来也不现实）
- 复制资源：内嵌 icon font、QSS 样式表、`.qm` 翻译、OCR 模型、应用图标、品牌名 `PixPin`
- 复用其账号/授权/服务端接口

**建议定位**："功能对标 + 自主 UI"，即"能截图、能录屏、有工具条、热键可改、设置齐全"，而不是像素级还原。UI 现用 egui 自绘，风格自己定。

---

## 7. 建议推进顺序

**阶段 0（1–2 天）— 清债**
1. 修文字标注不导出（当前是硬伤，demo 即穿帮）
2. 多显示器交互截图接上（`capture_monitor_by_index` 已就绪）
3. `StorageConfig` 落盘

**阶段 1（1 周）— 设置与工具栏地基**
- 统一配置层：JSON + 点分键 + 版本号 + 默认值表 + 迁移
- 数据驱动设置窗口（先做 5 个页面：截图/录制/热键/保存/外观）
- `toolbar.rs` 泛化为 Layer + 按钮描述表 + 浮层面板 + 锚点避让

> 理由：录屏工具条要复用这套，先做省一半工。

**阶段 2（3 周）— 截屏对齐**
- 标注工具补齐：放大镜、水印、橡皮、折线、自动打码
- 每工具样式记忆（对齐 `MarkBar.*` 那 60 个键的语义）
- 选区预设、圆角选区、尺寸显示选项
- 窗口/控件吸附（先做窗口级，控件级靠边缘检测）

**阶段 3（2 周）— 录屏 MVP**
- 新建 `rshot-record` crate：SCK 采集 → VideoToolbox H.264 → mp4
- 选区录制 + 全屏录制 + 麦克风单轨
- 无工具条、无叠加，先跑通"按热键 → 录 → 停 → 有文件能播"

**阶段 4（3 周）— 录屏完整体验**
- 录制工具条（计时/帧率/暂停/停止/音频/摄像头）
- 系统音频 + 麦克风混音
- 光标/按键/进度条回放叠加
- 崩溃恢复 + 录制中快捷键

**阶段 5（1 周）— 快捷键动作系统**
- 动作表（内置动作列表）+ 用户自定义绑定 + 冲突/占用检测
- 触发排队串行化
- 可选：命令行/脚本绑定

**阶段 6（持续）— 打磨**
- 首启引导、权限引导、托盘菜单、悬浮球、多语言

---

## 8. 仍未确定的问题（下一步可深挖）

1. **录制时的实际帧率与掉帧策略**：`hasFrameChanged` + `updateArea` 暗示有"变化区域差分省 CPU"的机制，需实测 CPU 占用才能确认
2. **回放叠加是"录时烧录"还是"导出时重绘"**：`PixMovie.playback.*` 命名像导出时重绘（可事后改光标样式），需动态验证
3. **多轨容器的具体格式**：是否真是 mp4 多轨，还是分离文件再合成（`createTrimmedAudioFile` 暗示后者）
4. **UI 区域检测**（截图时高亮窗口/控件边缘）：`libUiRegionDetector` + OpenCV `detect.prototxt/caffemodel`，精度与实现代价需单独评估
5. **跨屏选区的坐标归一化**：`MacCrossScreenImageWindow` 如何处理不同 scale factor 的屏幕

如需，我可以针对 1–3 做进一步动态取证（录一段、看产物），或对 `libPixScreenManager` / `libPixMovie` 做反编译级分析确认调用细节。

---

## 9. 证据附录

**包路径**
- 主程序 `/Applications/PixPin.app/Contents/MacOS/PixPin`
- 模块 `/Applications/PixPin.app/Contents/Frameworks/libPix*.dylib`
- 配置 `~/Library/Application Support/PixPin/Config/PixPinConfig.json`
- 日志 `~/Library/Application Support/PixPin/pixpin.log`

**取证命令**
```bash
file MacOS/PixPin                                   # Universal, 2 arch
otool -L Frameworks/libPixMovie.dylib                # SCK/AVFoundation/CoreAudio
nm -gU -C MacOS/PixPin | grep -E " [A-Z][A-Za-z0-9_]+::"   # 类名清单
strings -a MacOS/PixPin | grep -E "^MarkBar\."       # 样式键
strings -a Frameworks/libPixAVCodec.dylib | head     # FFmpeg 7.1.1 构建路径
defaults read com.depthpicture.PixPin                # 系统偏好（几乎空，配置在 JSON）
```

**关键符号（可复用的架构参照）**
- 截屏：`PixPin::screenShot` / `initScreenShotWindow` / `ScreenShotWindow` / `ScreenShotView` / `ScreenShotEdit`
- 录屏：`ScreenRecordWindow` / `ScreenPlaybackWindow` / `PixMovieUI::createMultiTrackRecorder` / `RecordingRecoveryManager`
- 工具栏：`ActionsBar`（12 个 add* 方法）/ `ActionsBarLayer` / `FloatPanel*` / `ToolBall`
- 热键：`QxtGlobalShortcut` / `GlobalShortcutAppClass` / `PixPinAction` / `PixPinActionManage`
- 设置：`PdConf::ConfWidget*`（17 个）/ `ConfItem` / `PixStorage`
