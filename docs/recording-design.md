# rshot 录屏方案（设计稿）

- 目标仓库：`rshot`（Rust，edition 2024，rustc 1.91）
- 目标平台：macOS 27.2 / arm64（当前机器），macOS 13+ 为最低支持线
- 对标：PixPin 录制模块（见 [pixpin-feasibility.md](pixpin-feasibility.md)）
- 状态：设计稿，未写代码

---

## 1. 范围

**要做：**
- 选区 / 全屏 / 窗口 三种录制目标
- 系统声音 + 麦克风双音轨
- 录制中浮动工具条（计时、帧率、暂停/继续、停止、音频开关）
- 光标录制（可选）、点击高亮（可选）
- 暂停 / 继续，无缝时间线
- 崩溃后可恢复（录到一半进程被杀，文件仍能播）
- 导出（可选叠加光标轨迹、按键气泡、进度条）

**不做（本方案外）：**
- 摄像头画中画、摄像头轨
- 剪辑器（裁剪/变速/合并）
- GIF 导出（复用截图 GIF 工具链另说）
- Windows / Linux 采集后端（留 trait，后续填）

---

## 2. 技术路线：三条，推荐第 2 条

macOS 上"把屏幕录成 mp4"有三条路，差别很大，必须先定：

| 路线 | 做法 | 优点 | 致命缺点 |
| --- | --- | --- | --- |
| **A. `SCRecordingOutput`** | SCK 自己写文件 | 约 100 行代码，2 天出活 | **只能设 file_url / codec / file_type，码率、关键帧间隔、帧率都不可控** |
| **B. SCK → `AVAssetWriter`** | 拿 CMSampleBuffer 喂 AVAssetWriter | 码率/关键帧/分段/多轨全可控；苹果维护；约 400 行 | 要写少量 objc2 unsafe |
| **C. SCK → VideoToolbox → 自写 mp4 muxer** | 完全自己来 | 极限控制 | 工作量 ×3，没有收益 |

已验证的 API 事实（决定选型的前提）：

```
screencapturekit 11.1.2
  SCRecordingOutputConfiguration 只有: output_url / video_codec / output_file_type
  → 无 bitrate、无 keyframe interval、无 fps（fps 只能由 SCStreamConfiguration 定）

objc2-av-foundation 0.3.2
  AVAssetWriter: additive 全部齐全
  AVAssetWriter.movieFragmentInterval   ← 分片 mp4，崩溃可恢复
  AVAssetWriterInput.appendSampleBuffer(&CMSampleBuffer)  ← SCK 的 buffer 可直接喂
  → 不需要 AVAssetWriterInputPixelBufferAdaptor
```

**结论：**

- **主线走 B（AVAssetWriter）**
- **A 只在第 1 天做一次 spike**，用来验证「权限 → 采集 → 出文件」链路通不通，之后留作**降级后端**（AVAssetWriter 在新系统上出问题时能立刻切回来）
- **C 不做**。只有将来要出非 mp4 容器（webm/avi）时才考虑

> 用 `CaptureBackend` trait 把 A/B 隔开，上层 UI 和状态机完全不用改。这是本方案唯一的"预先设计"，其余都保持简单。

---

## 3. 总体架构

### 3.1 crate 划分

```
rshot-record            新 crate，纯库，无 UI
├── backend/
│   ├── mod.rs          trait CaptureBackend + 后端选择
│   ├── sck_writer.rs   [主线] SCStream + AVAssetWriter
│   └── sck_output.rs   [降级] SCStream + SCRecordingOutput
├── session.rs          会话目录、临时文件、恢复扫描
├── target.rs           RecordingTarget（Display/Region/Window）→ SCContentFilter
├── permission.rs       屏幕录制 / 麦克风 / 辅助功能
├── events.rs           光标·点击·按键·滚动 采样 + jsonl 落盘
├── recorder.rs         状态机 + 命令通道 + 统计
└── compose.rs          导出叠加（读 mp4 → 合成 → 再编码）

rshot-record-ui         新 crate（或并入 rshot-overlay）
└── 浮动工具条（eframe/egui，复用 rshot-pin 的窗口套路）
```

依赖方向：

```
rshot-record-ui ──> rshot-record ──> rshot-core(clipboard/storage 可复用)
```

不要在 `rshot-core` 里放录屏——`core` 现在是"单帧图像"，录屏是"带时钟的流"，混进去会把 core 撑爆。

### 3.2 线程模型（**这是全部设计里最容易翻车的地方**）

```
┌──────────────────────────────────────────────────────────────┐
│ 主线程  winit/eframe event loop                              │
│   · 工具条 UI、按钮、计时显示                                 │
│   · 只做：发命令(cmd_tx)、收事件(evt_rx)，绝不做重活            │
└───────────┬──────────────────────────────▲──────────────────┘
            │ cmd_tx (crossbeam bounded)    │ evt_rx (unbounded stats)
            ▼                               │
┌───────────────────────────────┐          │
│ Recorder 控制线程              │──────────┘
│  · 状态机 Idle→Recording→…     │
│  · 启停 SCStream / 写 session │
└───────────┬───────────────────┘
            │ 启动
            ▼
┌──────────────────────┐   ┌──────────────────────┐
│ SCK dispatch queue    │   │ 事件采样线程          │
│ did_output_sample_    │   │ CGEventSource 轮询    │
│ buffer():             │   │ + CGEventTap(可选)    │
│ · 只打时间戳、入队     │   └──────────────────────┘
│ · 满则丢帧并计数       │
│ · 绝不阻塞、绝不编码   │
└──────────┬───────────┘
           │ bounded(4~8) 视频帧
           ▼
┌──────────────────────┐
│ 写盘线程（串行队列）    │
│ AVAssetWriterInput    │
│ .appendSampleBuffer() │
│ 返回 false = 背压       │
└──────────────────────┘
```

三条铁律：

1. **SCK 回调里只入队**。在回调里做编码/写盘会让系统直接给你降帧甚至断流，这是 ScreenCaptureKit 最经典的坑。
2. **有界队列 + 明确丢弃策略**。录制时宁可丢 1 帧，也不能让音频/视频时间戳乱了。丢帧要计数并上报（工具条上能看见"丢帧 N"）。
3. **所有 AVAssetWriter 调用在同一条串行 dispatch queue 上**。它非线程安全。

### 3.3 时钟

- 视频 PTS 直接取 `CMSampleBuffer` 的 presentation time，**不要自己打时间戳**
- 音频 PTS 同源（SCK 保证），混音/分轨时按 PTS 对齐
- 可用 `SCStream::synchronization_clock()` 拿 `CMClock` 做参考
- 暂停时不要用"停止计时器"的做法，走时间线压缩（见 §8）

---

## 4. 核心数据结构

```rust
// target.rs
pub enum RecordingTarget {
    /// 整块显示器
    Display(DisplayId),
    /// 显示器上的矩形（逻辑坐标，单位 points）
    Region { display: DisplayId, rect: Rect },
    /// 单个窗口（跟随窗口移动）
    Window { window: WindowId },
}

// recorder.rs
pub struct RecordingConfig {
    pub target: RecordingTarget,
    pub fps: u32,                      // 24 / 30 / 60
    pub quality: VideoQuality,         // 决定码率，见 §6.2
    pub codec: VideoCodec,             // H264 | HEVC
    pub audio: AudioConfig,
    pub cursor: CursorMode,            // None | Always | ClickOnly
    pub record_events: bool,           // 是否记录光标/按键（供导出叠加）
    pub out_dir: PathBuf,
}

pub struct AudioConfig {
    pub system: bool,
    pub microphone: Option<DeviceId>,
}

// recorder.rs
pub enum RecorderCmd {
    Pause,
    Resume,
    Stop,
    SetMic(bool),
    SetSystemAudio(bool),
}

pub enum RecorderEvent {
    Started { path: PathBuf, started_at: Instant },
    Ticked { elapsed: Duration, fps: f32, dropped: u64, bytes: u64 },
    Paused,
    Resumed,
    Warning(WarningKind),   // 磁盘不足 / 丢帧过多 / 显示器变化
    Finished(RecordingArtifact),
    Failed(String),
}

pub struct RecordingArtifact {
    pub path: PathBuf,
    pub duration: Duration,
    pub dropped_frames: u64,
    pub events_path: Option<PathBuf>,
    pub session_dir: PathBuf,
}
```

后端 seam：

```rust
pub trait CaptureBackend {
    fn start(&mut self, cfg: &RecordingConfig) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
    fn stop(self: Box<Self>) -> Result<RecordingArtifact>;
    /// 供工具条显示的实时指标
    fn stats(&self) -> BackendStats;
}
```

---

## 5. 采集层：SCK 配置

### 5.1 目标 → SCContentFilter

```rust
let content = SCShareableContent::get()?;           // 需要屏幕录制权限，否则返回空

let filter = match &cfg.target {
    RecordingTarget::Display(id) => {
        let d = find_display(&content, id)?;
        SCContentFilter::create()
            .with_display(&d)
            .with_excluding_windows(&excluded)      // ← 见 5.3
            .build()?
    }
    RecordingTarget::Region { display, .. } => {
        let d = find_display(&content, *display)?;
        SCContentFilter::create()
            .with_display(&d)
            .with_excluding_windows(&excluded)
            .build()?
    }
    RecordingTarget::Window(w) => {
        let win = find_window(&content, *w)?;
        SCContentFilter::create()
            .with_window(&win)
            .build()?
    }
};
```

### 5.2 SCStreamConfiguration：**points vs pixels 陷阱**

```rust
let mut sc = SCStreamConfiguration::new();
sc.set_width(px_w);           // 像素
sc.set_height(px_h);          // 像素
sc.set_fps(cfg.fps as i32);
sc.set_queue_depth(6);        // 内部队列，默认 3，录制建议 5~8

// 选区：source_rect 是「逻辑坐标 points」，不是像素
sc.set_source_rect(CGRect::new(
    CGPoint::new(rect.x, rect.y),
    CGSize::new(rect.w, rect.h),
));
// 输出分辨率必须自己按 scale 换算
sc.set_destination_rect(CGRect::new(CGPoint::ZERO, CGSize::new(px_w as f64, px_h as f64)));
sc.set_scales_to_fit(true);
sc.set_preserves_aspect_ratio(true);
sc.set_pixel_format(PixelFormat::BGRA);

sc.set_shows_cursor(matches!(cfg.cursor, CursorMode::Always | CursorMode::ClickOnly));
sc.set_shows_mouse_clicks(matches!(cfg.cursor, CursorMode::ClickOnly));  // macOS 15+

sc.set_captures_audio(cfg.audio.system);
sc.set_excludes_current_process_audio(true);   // 别把工具条提示音录进去
sc.set_captures_microphone(cfg.audio.microphone.is_some()); // macOS 15+
if let Some(dev) = cfg.audio.microphone { sc.set_microphone_capture_device_id(dev); }
```

**必须写进代码注释的一条**：`source_rect` 用 points，`width/height` 用 pixels。Retina 屏上这是 2 倍差，写错就是"录出来的画面糊一半"或者"录到了隔壁屏幕的一块"。换算：

```
px_w = round(rect.w * display.scale_factor)
```

而 `display.scale_factor` 要从 `SCDisplay` 拿（`SCContentFilter::point_pixel_scale()` 也能给）。

### 5.3 排除自身窗口

录制时工具条在屏幕上，不排除就会被录进去。两个动作：

1. `with_excluding_windows(&[toolbar_window_id])` —— 需要拿到 NSWindow → `CGWindowID`
2. `set_excludes_current_process_audio(true)` —— 不录自己的提示音

工具条窗口建议直接 `NSWindowLevel = NSStatusWindowLevel` + `sharingType = .none`，双保险（`NSWindow.sharingType = NSWindowSharingNone` 能让 SCK 天然看不到它）。

### 5.4 显示器/分辨率变化的处理

录制中如果用户改了分辨率、插拔显示器、切全屏：

- 监听 `NSApplicationDidChangeScreenParametersNotification`
- 策略：**不做热重配**（SCK 的 `update_configuration` 能改，但改动期间的时间戳会有断点）
- 做法：发 `RecorderEvent::Warning(ScreenChanged)` → 自动停止并保存已有内容，提示"因屏幕变化已停止录制"

这条比"努力适配"正确得多。

---

## 6. 编码与封装：AVAssetWriter

### 6.1 建 writer

> 下列代码为**结构示意**（伪代码），`dict!` 是示意宏。实际用 `objc2-foundation` 的
> `NSDictionary::from_slices` + `ns_string!` 构造 settings 字典，常量名已核验：
> `AVFileTypeMPEG4` / `AVVideoCodecTypeH264` / `AVVideoCodecTypeHEVC` /
> `AVAssetWriterInput::initWithMediaType_outputSettings` / `setExpectsMediaDataInRealTime`。

```rust
let writer = AVAssetWriter::assetWriterWithURL_fileType_error(
    &url,
    AVFileType::MPEG4,      // .mp4；要 .mov 用 AVFileTypeQuickTimeMovie
)?;

// 崩溃恢复的关键：每 1 秒落一个 moof 片段
writer.set_movieFragmentInterval(CMTime::new(1, 1));
writer.setShouldOptimizeForNetworkUse(false);

// 视频
let video_settings = dict! {
    "AVVideoCodecKey" => "avc1",                       // 或 "hvc1" (HEVC)
    "AVVideoWidthKey" => px_w,
    "AVVideoHeightKey" => px_h,
    "AVVideoCompressionPropertiesKey" => dict! {
        "AVVideoAverageBitRateKey" => bitrate,
        "AVVideoExpectedSourceFrameRateKey" => fps,
        "AVVideoMaxKeyFrameIntervalKey" => fps * 2,     // 2 秒一个 I 帧
        "AVVideoProfileLevelKey" => "AVVideoProfileLevelH264HighAutoLevel",
        "AVVideoAllowFrameReorderingKey" => true,
    },
};
let video_in = AVAssetWriterInput::initWithMediaType_outputSettings(
    AVMediaTypeVideo, &settings,
)?;
video_in.setExpectsMediaDataInRealTime(true);          // ← 实时源必设
writer.addInput(&video_in);
```

音频同理：

```rust
let audio_settings = dict! {
    "AVFormatIDKey" => kAudioFormatMPEG4AAC,
    "AVSampleRateKey" => 48000,
    "AVNumberOfChannelsKey" => 2,
    "AVEncoderBitRateKey" => 192_000,
};
```

### 6.2 码率表

| 分辨率 | fps | H.264 | HEVC |
| --- | --- | --- | --- |
| ≤1280×720 | 30 | 4 Mbps | 3 Mbps |
| 1920×1080 | 30 | 8 Mbps | 6 Mbps |
| 1920×1080 | 60 | 14 Mbps | 10 Mbps |
| 2560×1440 | 30 | 14 Mbps | 10 Mbps |
| 3840×2160 | 30 | 28 Mbps | 20 Mbps |
| 3840×2160 | 60 | 45 Mbps | 32 Mbps |

实现为 `VideoQuality::{Low, Medium, High, Lossless}` → 按上表 + 屏幕像素数取整。**这就是路线 A 做不到的东西**（SCRecordingOutput 没有码率参数）。

### 6.3 写盘的背压处理

```rust
fn on_video_frame(&self, sb: CMSampleBuffer) {
    if !self.video_in.isReadyForMoreMediaData() {
        self.dropped.fetch_add(1, Relaxed);
        return;                    // 丢帧，不阻塞 SCK
    }
    if !self.video_in.appendSampleBuffer(&sb) {
        self.dropped.fetch_add(1, Relaxed);
    }
}
```

`isReadyForMoreMediaData == false` 时**必须丢**，不能 `while` 等——等就会堵住 SCK 的 dispatch queue。

### 6.4 收尾

```rust
video_in.markAsFinished();
audio_sys_in.markAsFinished();
if let Some(mic) = mic_in { mic.markAsFinished(); }
writer.endSessionAtSourceTime(session_end);
writer.finishWritingWithCompletionHandler(|| { /* 信号完成 */ });
// 拿到最终文件大小、duration
```

`finishWriting` 是异步的，**必须等回调**再报 `Finished`，否则用户点"停止"后立刻 `open` 文件会看到 0 字节。

---

## 7. 音频

### 7.1 两路来源

| 来源 | 拿到方式 | 备注 |
| --- | --- | --- |
| 系统声音 | SCK `SCStreamOutputType::Audio` | 需要 `captures_audio(true)`，macOS 13+ |
| 麦克风 | SCK `SCStreamOutputType::Microphone` | macOS 15+；老系统回落到 `cpal` |

因为本机是 macOS 27，直接走 SCK 双路，**不需要 cpal**。cpal 只在支持 macOS 13/14 时做兜底，可以先不做。

### 7.2 两轨还是混音？

**第一阶段：两条独立音轨**（两个 `AVAssetWriterInput`）。

- 简单、无时钟对齐问题、后期可分别调音量
- 代价：mp4 里两个 audio track，部分播放器只播第一条

**第二阶段（导出时）**：合并成一条。PixPin 就是这么干的——`PixMovieUI::createTrimmedAudioFile` 先把每轨裁出来，再合成。我们照这个思路：录制时留双轨，导出/保存时混成单轨。

### 7.3 麦克风权限

```rust
// AVCaptureDevice.authorizationStatus(for: .audio)
// 未决定 → requestAccess；拒绝 → 工具条上麦克风按钮置灰并给"去设置"入口
```

不要静默失败——用户会以为录上了声音。

---

## 8. 暂停 / 继续

**不能简单停表**，否则视频时间线会出空洞或者加速。

方案：**时间线压缩（timestamp compaction）**

```
[录 0~10s]  暂停 5s  [继续 录 15~20s]
最终文件时长 = 15s，不是 20s
规则：resume 后每个 PTS 减去累计暂停时长
```

实现：

```rust
struct ClockAdjust {
    accumulated_pause: CMTime,
    pause_started: Option<CMTime>,   // 主机时间
}

fn adjust(&self, pts: CMTime) -> CMTime {
    pts - self.accumulated_pause
}
// pause:  pause_started = Some(now())
// resume: accumulated_pause += now() - pause_started
```

注意：暂停期间**仍然要消费 SCK 的帧并丢弃**（不能停流），否则 resume 后第一帧的时间戳会跳变，且 SCK 恢复有 100~300ms 延迟。丢弃成本很低。

停止时 `endSessionAtSourceTime` 要用**压缩后**的时长。

---

## 9. 事件记录（为导出叠加服务）

PixPin 的 `PixMovie.playback.*` 那一堆配置（光标样式、点击动画、滚轮、按键显示、进度条）说明它是**导出时重绘**，不是录时烧录。我们照做——这样用户录完还能改光标样式。

### 9.1 采样内容

| 事件 | 采集方式 | 需要权限 |
| --- | --- | --- |
| 光标位置 | `CGEventSource` 25~30Hz 轮询 `CGEventCreate(NULL)` 取 `kCGMouseEventLocation` | **不需要** |
| 点击 | `CGEventTap` 监听 leftMouseDown/rightMouseDown | 辅助功能 |
| 滚动 | `CGEventTap` 监听 scrollWheel | 辅助功能 |
| 按键 | `CGEventTap` 监听 keyDown（只记 keycode + 修饰键，**不记字符**） | 辅助功能 |

**关键取舍**：光标位置是叠加效果的主力（占 90% 观感），而它**不需要任何权限**。所以：

- 没有辅助功能权限 → 光标轨迹照常可用，只是没有点击涟漪和按键气泡
- 工具条上按键那个开关直接置灰 + 提示，不要弹权限框打扰

### 9.2 落盘格式

`{session}/events.jsonl`，逐行追加，崩溃也不丢（最多丢最后一行）：

```jsonl
{"t":0,"k":"rec_start"}
{"t":125,"k":"move","x":840,"y":512}
{"t":133,"k":"move","x":844,"y":515}
{"t":2010,"k":"click","btn":"left","x":844,"y":515}
{"t":3400,"k":"key","code":12,"mods":["cmd"]}
{"t":5000,"k":"pause"}
{"t":5010,"k":"rec_end"}
```

`t` 用毫秒，**并且必须是压缩后的时间线**（和 §8 同一套时钟），否则导出时音画不同步。

坐标系：统一用**录制区域左上角为原点的逻辑像素**。多屏时先换算到全局坐标再减区域原点。

### 9.3 采样开销

25Hz 的 `CGEventCreate(NULL)` 很便宜（微秒级），但要注意：

- 跑在独立线程，用 `std::thread::sleep` 而非 timer 事件（不依赖主线程）
- 只在光标位置变化 > 1px 时写一行，否则文件会很大（25Hz × 10 分钟 = 15000 行，可接受；但如果每次都写会到 1.5 万行，也还好。先做变化过滤更稳）

---

## 10. 导出叠加（compose）

这是"看起来专业"的关键，也是工作量藏在后面的部分。

### 10.1 管线

```
原 mp4 ──AVAssetReader──> CVPixelBuffer 帧
                              │
events.jsonl ────────────────>│ 找到该 PTS 对应的光标位置/点击/按键状态
                              ▼
                    CGContext / Metal 合成
                    · 光标箭头（SVG 或位图，可缩放）
                    · 点击涟漪（半径随 age 衰减）
                    · 按键气泡（圆角矩形 + 键名，队列式）
                    · 进度条 / 水印
                              │
                              ▼
                    AVAssetWriterInput (H.264) ──> 新 mp4
```

### 10.2 为什么不复用录制时的 writer

导出是**离线重编码**，可以用更高码率、可以两遍编码、可以精确 seek。和实时录制共用 writer 只会互相拖累。分成两个独立管线，共享的只有 `compose` 的绘制函数。

### 10.3 分阶段

- **P0**：不导出，直接给原 mp4（B 路线下光标可由 SCK 直接录进去，够用）
- **P1**：导出时加光标轨迹 + 点击涟漪（纯 CG 绘制，不用 Metal）
- **P2**：加按键气泡 + 进度条

先做 P0 上线，P1 按需排期。

---

## 11. 工具条与交互

### 11.1 窗口形态

- 无边框、圆角、半透明深色（和截图工具条同一套视觉）
- `NSWindowLevel = NSStatusWindowLevel`
- `NSWindow.sharingType = NSWindowSharingNone`（不被自己录进去）
- 可拖动（复用 rshot-pin 的 `MoveControl` 思路）
- 记住位置（写配置）

### 11.2 按钮（对齐 PixPin 的录制条）

```
[● 00:12:34] [30fps] [暂停/继续] [系统声 🔈] [麦克风 🎤] [摄像头(暂不做)] [停止]
```

- 计时器：`RecorderEvent::Ticked` 驱动，1Hz 刷新（不要 60Hz 重绘数字）
- 帧率：实际写入帧率，掉到设定值 80% 以下时变黄
- 丢帧数：> 0 时在 tooltip 里显示
- 停止：先 `Cmd` 结束 → 等 `finishWriting` → 弹"已保存，点击预览"

### 11.3 热键

沿用现有 `global-hotkey`：

- `Cmd+Shift+R` 开始/停止录制（默认）
- `Esc` 停止录制（录制中）
- 录制中禁用其它截图热键（PixPin 的 `disableShortcuts` 也是这个思路）

### 11.4 启动流程

```
热键 → 进程 A（--record）
        ├─ 检查权限（屏幕录制 / 麦克风）
        ├─ 冻结屏幕 + 选区覆盖层（复用 rshot-overlay 的选区逻辑！）
        ├─ 用户确定区域 → 创建 session → 启动 Recorder
        └─ 弹出工具条
```

**复用点**：选区交互完全复用 `rshot-overlay` 的选区状态机（`crates/rshot-overlay/src/state.rs`），只需要换一个"确认后的动作"，从"裁剪图片"改成"把矩形交给 Recorder"。这是本方案能省最多时间的地方。

---

## 12. 权限与失败处理

### 12.1 权限三件套

| 权限 | 检查 | 申请 | 拒绝后 |
| --- | --- | --- | --- |
| 屏幕录制 | `CGPreflightScreenCaptureAccess()` | `CGRequestScreenCaptureAccess()` | 明确提示 + 打开系统设置深链 |
| 麦克风 | `AVCaptureDevice.authorizationStatus(.audio)` | `requestAccess` | 麦克风按钮置灰 |
| 辅助功能（可选） | `AXIsProcessTrustedWithOptions` | 同上 | 按键/点击叠加不可用 |

已有 `ScreenCapture::has_screen_recording_permission()`（`rshot-core/src/screenshot/mod.rs`）可直接复用。

**重要**：命令行直接跑的二进制没有自己的 TCC 身份，权限落在终端/IDE 上。所以录制功能必须**打包成 .app** 才能正常授权给用户，这是产品化前提。已有 `responsible_app_name()` 可以把这个情况提示出来。

### 12.2 失败场景清单

| 场景 | 行为 |
| --- | --- |
| 权限被拒 | 不启动，弹引导，不留下空文件 |
| 磁盘 < 500 MB | 开始前拒绝；录制中降到 200 MB 时发 Warning 并自动停止 |
| 显示器配置变化 | 自动停止 + 保存已有内容 |
| 录制中锁屏 | 继续录（SCK 仍工作）但发 Warning |
| 编码器初始化失败 | 切降级后端（SCRecordingOutput）重试一次 |
| `appendSampleBuffer` 持续 false 超 2 秒 | 记 Warning，继续丢帧，不中断 |
| 进程被杀 | fMP4 保证最后 1 秒之前的内容可播；启动时扫 session 目录提示恢复 |

---

## 13. 性能预算与验收指标

| 项目 | 1080p30 | 4K30 |
| --- | --- | --- |
| CPU（硬编） | < 8% | < 20% |
| 内存峰值 | < 150 MB | < 350 MB |
| 磁盘写入 | ~1 MB/s | ~3.5 MB/s |
| 首帧延迟（点开始→画面入文件） | < 400 ms | < 600 ms |
| 停止延迟（点停止→文件可用） | < 1.5 s | < 2.5 s |
| 长时间稳定性 | 连续 2 小时不掉帧率、内存不涨 | 同左 |

压测项（每个都要有自动化脚本）：
1. 连续录 2 小时，采样 RSS 与 dropped 计数
2. 录制中切分辨率 / 插拔显示器，验证优雅停止
3. 录制中 `kill -9`，验证 fMP4 可播、恢复提示正确
4. 录制中磁盘写满，验证停止且文件可用
5. 30 次连续启停，验证无 fd/内存泄漏

---

## 14. 目录布局与命名

```
~/Movies/rshot/
└── 2026-11-13_15-30-00_a1b2c3/        # session_id = 时间戳_随机
    ├── session.json                    # 配置快照 + 状态（recording/finished/crashed）
    ├── events.jsonl                    # 事件流
    ├── recording.mp4                   # 最终产物（fMP4）
    └── .partial/                       # 录制中，完成即删
```

- 录制中文件写 `.partial/recording.mp4`，`finishWriting` 成功后才移到外层并改 `session.json` 状态
- 启动时扫描 `session.json` 里状态是 `recording` 的目录 → 提示"上次录制未正常结束，已恢复 X 秒"

文件名模板对齐 PixPin 的 `$yyyy-MM-dd_HH-mm-ss$` 语义，用 `rshot-storage` 的现有格式函数。

---

## 15. 阶段拆分与验收

| 阶段 | 内容 | 工期 | 验收标准 |
| --- | --- | --- | --- |
| **R0** | 权限检查 + 全屏录制 + SCRecordingOutput（spike） | 2 天 | 命令行敲一条，5 秒后得到一个能播的 mp4 |
| **R1** | `CaptureBackend` trait + AVAssetWriter 后端 + 全屏录制 | 3 天 | 同 R0，但码率可控；`ffprobe` 看到目标码率与关键帧间隔 |
| **R2** | 选区录制（复用 overlay 选区）+ 系统声音 | 3 天 | 框一块区域录，画面不糊、不串屏、有系统声 |
| **R3** | 录制工具条 + 暂停/继续 + 计时/帧率 | 3 天 | 暂停 5 秒后继续，成品时长正确、音画同步 |
| **R4** | 麦克风 + 双音轨 + 权限降级 | 2 天 | 双轨都在；拒绝麦克风权限时按钮置灰且不崩 |
| **R5** | 崩溃恢复 + 边界处理 + 压测脚本 | 3 天 | `kill -9` 后能播能恢复；5 项压测全过 |
| **R6** | 事件记录（光标/点击/按键） | 3 天 | `events.jsonl` 完整；光标坐标与画面吻合 |
| **R7** | 导出叠加 P1（光标轨迹 + 点击涟漪） | 4 天 | 导出视频里光标轨迹平滑，无拖影 |
| **R8** | 降级后端收尾 + 打包 .app + 首启引导 | 2 天 | 双击 .app 能正常授权并录制 |

**合计约 25 个工作日**（不含 R7 的话 21 天）。其中 R0–R3 是"能用"的最小闭环，约 11 天。

**关键里程碑判断**：R1 结束时如果 AVAssetWriter 这条路走不通（编码失败、时间戳异常），立刻退回 R0 路线，R2 之后的 UI 与状态机代码完全不用改——这正是 `CaptureBackend` seam 的价值。

---

## 16. 风险表

| 风险 | 概率 | 影响 | 缓解 |
| --- | --- | --- | --- |
| SCK 回调阻塞导致系统降帧 | 高 | 录制掉帧、画面卡 | 回调只入队；有界队列；丢帧计数可见 |
| `source_rect` 单位搞错 | 高 | 录错区域/画面糊 | 单测覆盖 points↔pixels；R2 必须有 Retina + 非 Retina 双屏测试 |
| AVAssetWriter 时间戳不连续报错 | 中 | 录制中断 | 严格用 CMSampleBuffer PTS；暂停走压缩时钟；失败切降级后端 |
| 麦克风与系统声时钟漂移 | 中 | 长录后音画不同步 | 双轨不混音；导出时才混；<30 分钟不混 |
| 辅助功能权限拿不到 | 高 | 按键叠加不可用 | 明确降级，不阻断主流程 |
| 工具条被录进去 | 中 | 画面有瑕疵 | `sharingType = .none` + `with_excluding_windows` 双保险 |
| 新 macOS 改 SCK API | 中 | 编译或运行失败 | `screencapturekit` 有 `macos_13_0`~`macos_26_0` feature 分层；锁定版本 |
| 命令行运行无法获得 TCC 授权 | 高 | 开发期一直空白 | 开发期用打包的 .app 调试；文档写清楚 |

---

## 17. 与现有 rshot 的集成点

改动很小，因为录屏是独立 crate：

1. **`src/main.rs`**：加 `--record` / `--record-fullscreen` / `--record-region x,y,w,h` 三个参数，`--record` 走子进程（沿用现在 `spawn_capture` 的套路，避免和托盘的事件循环打架）
2. **`crates/rshot-tray/src/lib.rs`**：菜单加"开始录制"，`CaptureKind` 加 `Record` 变体
3. **`crates/rshot-tray/src/config.rs`**：`Hotkeys` 加 `record: String`，默认 `Cmd+Shift+R`
4. **`crates/rshot-overlay`**：选区状态机抽出一个可复用的 `SelectionController`，录制时也用；确认动作由回调注入
5. **`Cargo.toml`**：加 `rshot-record`、`rshot-record-ui` 两个 workspace member

不需要动的：`rshot-core`、`rshot-storage`（保存路径函数可复用）、`rshot-pin`。

---

## 18. 跨平台说明

本方案是 **macOS 优先**。跨平台不是"再写一遍"，而是：

- `CaptureBackend` trait 已经是平台无关的
- Windows：Windows.Graphics.Capture + Media Foundation（`windows` crate），工作量与 macOS 相当
- Linux X11：XShm 抓帧；Wayland：PipeWire（需要 portal），体验会打折

**明确建议**：macOS 版本的 R0–R5 全绿之前，不要开 Windows/Linux 分支。否则三个平台的坑会互相拖累。

---

## 19. 依赖清单（已核验版本）

```toml
[target.'cfg(target_os = "macos")'.dependencies]
screencapturekit       = { version = "11.1.2", features = ["macos_15_0"] }  # SCK + SCRecordingOutput
objc2                  = "0.6"
objc2-foundation       = "0.3"
objc2-av-foundation    = "0.3.2"   # AVAssetWriter / AVAssetReader（已验证含 movieFragmentInterval、appendSampleBuffer）
objc2-core-media       = "0.3"     # CMTime / CMSampleBuffer
objc2-core-video       = "0.3"     # CVPixelBuffer
objc2-core-graphics    = "0.3"     # 已有，权限与光标坐标
objc2-app-kit          = "0.3"     # NSWindow sharingType / level
block2                 = "0.6"

[dependencies]
crossbeam-channel      = "0.5"     # bounded 队列
serde / serde_json     = "1"       # session.json / events.jsonl
thiserror              = "2"
log                    = "0.4"
```

**不引入**：`ffmpeg-next`（体积与编译时间不值）、`cpal`（SCK 已覆盖）、`mp4`（AVAssetWriter 已覆盖）、`rubato`（不混音就不需要）。

---

## 20. 开工第一件事

如果现在就动手，第一步不是写采集，而是写这个：

```bash
# 一个 40 行的 bin，验证本机 SCK 能出帧
cargo run -p rshot-record --example probe
# 期望输出：
#   displays: 1  (1920x1080 @ 2.0x)
#   screen capture permission: granted
#   frames in 3s: 90  (30.0 fps)
#   audio buffers in 3s: 150
```

这个 probe 通了，后面全是工程问题；probe 不通（权限、TCC、版本），先解决环境问题，别写业务代码。
