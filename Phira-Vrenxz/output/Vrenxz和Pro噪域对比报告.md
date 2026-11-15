---
AIGC:
    Label: "1"
    ContentProducer: 001191440300708461136T1XGW3
    ProduceID: 2e054e3e197ddb1982b9e115794afa50_e923de69c16811f18019525400248c00
    ReservedCode1: Ae0jJJTk3ltm9/cM2M97lunMKyk6ptcLm6xxV/b1KosedR4vKFnsvjluykfb6VNIa60Njm467NyqW+eYGnUOne+eqjiStVaRMVYT/w7EwYpovhjATNDHMbLluf8ghkJLo4xkJhlo/dP62a0sagxr04Flujg0LzNUspBDXU9dy3kh+3iKobqeYGsCvqA=
    ContentPropagator: 001191440300708461136T1XGW3
    PropagateID: 2e054e3e197ddb1982b9e115794afa50_e923de69c16811f18019525400248c00
    ReservedCode2: Ae0jJJTk3ltm9/cM2M97lunMKyk6ptcLm6xxV/b1KosedR4vKFnsvjluykfb6VNIa60Njm467NyqW+eYGnUOne+eqjiStVaRMVYT/w7EwYpovhjATNDHMbLluf8ghkJLo4xkJhlo/dP62a0sagxr04Flujg0LzNUspBDXU9dy3kh+3iKobqeYGsCvqA=
---

# 噪域（Block Area）实现对比报告

- 对比对象：A = 本项目 `F:\phirLie`（prpr 引擎 + Phira-Vrenxz），B = 参照项目 `C:\Users\Administrator\Doubao\chats\2026-09-19\new-chat\Phira-Pro-main`
- 对比基准：prpr/src 噪域核心面 + assets 贴图 + 判定/音频/渲染接入点
- 日期：2026-10-06

---

## 0. 结论速览

1. **判定核心与官方语义零改动**：`block.rs` 两项目 MD5 完全一致，`BlockPhase` / `BlockArea` / `phase()` / `is_subtract` / even-odd（`block_touch_blocked_iter`）与 B 逐字节相同，官方 Phigros 触摸失效语义完整保留（用户强调项已重点核对，无差异）。
2. **完全一致的文件**：`block_simple.rs`、`block_timeline.rs`、`block_shader_full.vert`、`noise.glsl` 与 B MD5 一致；`block_mask.rs`、`block_touch.rs` 与 B 仅差异在 BOM 头与贴图路径（`assets/blockarea/` → `assets/textures/`）。
3. **适配必需差异 3 类**：① `glCopyTexImage2D` 替代 `glCopyTexSubImage2D`（定制 miniquad fork 缺后者）；② 音频 `set_amplifier + set_low_pass(0.0)` 模拟 B 的 `try_set_low_pass_filter(Hz)`（sasa rev e76229b 无 Hz API）；③ 贴图目录改为 `assets/textures`。
4. **A 独有优化 3 类**：① `zone_on_screen` 屏幕外剔除；② Reveal 全景缩小（`REVEAL_ZOOM_OUT=2.5`）下 `current_zone_scale / scale_zones_for_render` 缩放 Zone 并重剔除；③ 判定侧 `noise_finger_ids / invalid_finger_ids / held_by_noise` 显式断判。
5. **需跟进的潜在差异 4 项**：① `block_shader_full.frag` 三处计算简化（双路 displace→单路、双点 hash→单点）观感可能变"块状"，建议截图回归；② `prepare_block_geometry` 预热在 A 无调用点（首帧 CPU mask 生成推迟）；③ A 多出 `DisplaceMap.png / GlitchMap.png` 两张贴图但 src 无引用，疑似闲置；④ B 有 6 个 block_area 调试/基准示例，A 无。

---

## 1. 文件面对比

| 文件（prpr/src/core/） | B | A | 差异说明 |
|---|---|---|---|
| `block.rs` | ✓ | ✓ | **MD5 一致**。BlockPhase/BlockArea/phase/is_subtract/block_touch_blocked_iter 零改动 |
| `block_audio.rs` | ✓ | ✓ | 音频参数差异（见 §4） |
| `block_mask.rs` | ✓ | ✓ | 仅 BOM + 贴图路径差异；mask 合成逻辑一致 |
| `block_shader.rs` | ✓ | ✓ | A 新增 `zone_on_screen`；`glCopyTexImage2D` 替代；贴图路径；其余一致 |
| `block_shader_full.frag` | ✓ | ✓ | A 简化 3 处 displace/hash 计算（见 §3.2），uniform/阶段结构全保留 |
| `block_shader_full.vert` | ✓ | ✓ | **MD5 一致** |
| `block_simple.rs` | ✓ | ✓ | **MD5 一致** |
| `block_timeline.rs` | ✓ | ✓ | **MD5 一致** |
| `block_touch.rs` | ✓ | ✓ | 仅 BOM + 贴图路径差异 |
| `noise.glsl` | ✓ | ✓ | **MD5 一致** |
| 接入点 `scene/game.rs` | 1681 直接调 | 2110 于 `with_model(Reveal 缩放)` 内调 | A 多 Reveal 缩放适配 |
| 判定接入 `judge.rs` | `finger_blocked` + `touches.retain` | `noise_finger_ids / invalid_finger_ids / held_by_noise` | 机制不同、语义等价（见 §2） |
| `prepare_block_geometry` 预热 | game.rs:613 调用 | 仅导出未调用 | A 缺失预热（见 §6.2） |
| examples | 6 个 block_area 基准/调试 | 无 | 开发期调试工具缺失（见 §6.4） |
| assets | `blockarea/` 4 张 | `textures/` 6 张 | A 多 DisplaceMap/GlitchMap（见 §3.4） |

---

## 2. 判定逻辑对比（重点）

| 项目 | B（Phira-Pro） | A（Phira-Vrenxz） | 语义 |
|---|---|---|---|
| 区块命中 | `chart.touch_blocked(p, t, aspect)` → even-odd + inset，block.rs 一致 | 同（同一份 block.rs） | **一致** |
| 触摸失效生命周期 | `finger_blocked(&mut infected, id, phase, inside)`：Ended/Cancelled 移除，inside 感染 | `noise_finger_ids` + live_ids 收尾（等价移除）；touch id 在 `invalid_finger_ids` 中跳过判定 | **一致**（A 为独立实现） |
| 被感染手指处置 | `touches.retain` 直接移出触摸列表，其持有 Note 自然 miss | 保留在 touches 但 `continue` 跳过一切 Note 判定；失效手指负责的 Hold 由 `held_by_noise` 立即断判 Miss；Drag 驻留判定排除失效手指 | **等价**，A 更显式 |
| y 翻转 | 无此坑（原版坐标体系） | `hit_test` 入参 `ndc.y` 取负、红圈绘制 `pos.y` 取负，与主判定循环 `Point::new(p.x,-p.y)` 对齐 | A 修复了移植引入的翻转缺陷 |
| is_subtract / even-odd | 官方规则 | 与 B 同一文件 | **完全一致（用户强调项）** |

结论：A 未改任何官方判定常量与规则，仅换用自建感染集合与显式断判路径，语义与 B 等价且对 Hold 断判更主动。

---

## 3. 渲染对比

### 3.1 结构层面（一致）

- `block_shader_full.frag` 的 **76 个 uniform、6 个 sampler**（uDisplaceTex/uSparkTex/uMasks/uScene/uAuxMasks/uNoiseTex）两项目声明完全一致，`_SparkMap*` 参数全部保留（A 未删参数，仅单路采样）。
- 合成阶段一致：uMasks RGBA=(active Compose, Edge, Glow, disabled Compose)；uAuxMasks RGB=(ready-only Normal.r, ready-only Subtract.g, composedDisabled.r)；Hover 用 1/8 网格 + refined Compose 用 effect 网格双 pass。
- 参数表一致（如 `_BackgroundPixelScale=6.`、`_TouchBackgroundPixelScale=8.`、`_SparkMapOpacity=5.69`），diff 无差异。
- mask 数据流：`draw_layer_at` → `frame.masks.render_displaced(width, height, aspect, zones, time)` 两项目逐行一致（含 GLES3 blit 降采样路径）。

### 3.2 计算简化（A 对官方 shader 的 3 处改写）

| 位置 | B（官方反编译） | A | 观感影响 |
|---|---|---|---|
| Disabled 分支 displace | 两个偏移 UV（`u_xlat2.zw` + `u_xlat2.xy`）各采样一次后平均 | 仅 `u_xlat2.xy` 单点采样 | displace 平滑度降低，边缘可能略锐/略抖 |
| Active 分支 displace | 先按 `_ScreenParams` 换算像素、floor 像素对齐后再双路采样平均 | 直接 UV 单路采样（`u_xlat1.xy * _ScreenParams.zw`） | 像素级确定性略降，缩放/时间推进下细微闪烁差异 |
| 主噪声 hash | 双 hash 点（`u_xlat6 / u_xlat11`）lerp 插值（value noise） | 单 hash 点 | 噪声场失去插值平滑，SDF 边缘更"块状"，无渐变 |

> 其余计算路径（Touch SDF、Noise SDF、Spark、Edge/Glow 合成、SceneColor 采样、Compose/Subtract blender）与 B 一致。以上 3 处是 A 为减少采样数/规避 fork 编译问题做的简化，**不影响判定，只影响视觉细节**；官方观感如需完全还原，可将双路/双点恢复（每像素多 3 次 texture2D）。

### 3.3 剔除与缩放（A 独有）

- `zone_on_screen(zone, aspect)`：A 在 `block_shader.rs` 新增，`chart.rs block_zones()` 渲染时过滤屏幕外 Zone，B 无此剔除（判定不受影响，`hit_test` 不走该函数）。
- Reveal 全景揭晓：A 的 `game.rs` 对整个谱面模型栈缩放 1/2.5，噪域 mask 在全屏 NDC 空间合成（官方 shader 无 Model uniform），因此 A 在 `chart.rs` 用 `current_zone_scale` 读模型栈均匀缩放、`scale_zones_for_render` 缩小 Zone 并重做 `zone_on_screen` 剔除。B 无此模式。
- 噪域模型矩阵 `(flip_x?-1:1, -1)` 两项目一致。

### 3.4 贴图资源

| 贴图 | B `assets/blockarea` | A `assets/textures` |
|---|---|---|
| BlockNoise1.png | 84039 B | 84039 B（一致） |
| FD_Noise_00000.png | 89336 B | 89336 B（一致） |
| PointNoise.png | 12050 B | 12050 B（一致） |
| TouchHover.png | 1056 B | 1056 B（一致） |
| DisplaceMap.png | 无 | 98315 B，**A src 无引用**（疑似闲置/其它模块） |
| GlitchMap.png | 无 | 306524 B，**A src 无引用**（疑似闲置/其它模块） |

> uDisplaceTex 绑定 BlockNoise1.png（DISPLACE_TEX），两项目一致；DisplaceMap/GlitchMap 与噪域无关但建议核查归属。

---

## 4. 音频对比

| 维度 | B | A |
|---|---|---|
| 调用 | `music.try_set_low_pass_filter(cutoff, q, seconds)`（Hz 参数，带平滑） | `set_amplifier(0.35 / 1.0)` + `set_low_pass(0.0)` |
| 语义 | 官方：噪域内低通滤波（cutoff 降低高频） | 以"噪域内音量降低 + 低通归零"模拟官方"噪域内发闷"听感 |
| 适配背景 | sasa 提供 Hz API | sasa rev e76229b 无 Hz API（`set_low_pass(f32)` 无 `try_set_low_pass`），故改用放大器模拟 |

评估：A 的模拟方向合理（噪域内音量衰减 + 低通），但**不是严格等价**：官方 cutoff/q 曲线随 `_NoiseRadius` 平滑过渡，A 的 0.35/1.0 是硬切换近似。若后续 sasa 升级提供 Hz API，建议改回 `try_set_low_pass_filter` 以还原官方听感。

---

## 5. 适配差异逐项核对（已知 3 处 + 补充）

| # | 差异 | B | A | 是否影响表现 |
|---|---|---|---|---|
| 1 | `glCopyTexImage2D` 替代 `glCopyTexSubImage2D` | 使用后者 | 使用前者 | 否（fork GL 表缺后者；功能等价，均完成 RT 拷贝，采样一致） |
| 2 | 音频 API | `try_set_low_pass_filter(Hz)` | `set_amplifier + set_low_pass(0.0)` | 轻微（听感近似，非严格等价，见 §4） |
| 3 | game.rs 借用冲突 | 直接调 `render_block_overlay` | 包在 `with_model` 闭包内（Reveal 缩放） | 否（借用于 `res.with_model` 内，闭包结束即释放；非 Reveal 时缩放为 1，行为与 B 相同） |
| 4 | 判定侧 y 翻转修复 | 无此问题 | `hit_test`/红圈取负 | A 修正移植缺陷，行为更正确 |

---

## 6. 潜在遗漏 / 需跟进项

1. **官方 shader 观感还原（中）**：frag 三处简化（双路→单路、双点→单点）降低噪声平滑度，建议 A/B 同谱同帧截图对比；如观感差异明显，恢复官方采样路径。
2. **prepare_block_geometry 预热缺失（低-性能）**：B 在 `shader_pre_render` 时预生成 CPU mask 几何、规避首帧卡顿；A 该函数仅导出未调用，首帧 mask 生成推迟到 `draw_layer_at`，功能等价但首帧可能更慢。
3. **闲置贴图（低）**：`DisplaceMap.png / GlitchMap.png` 在 A 的 prpr/src 无引用，建议确认归属（可能供其它模块或已废弃）。
4. **调试示例缺失（低-开发）**：B 的 `examples/block_area_*`（benchmark/frames/gpu/mask_gpu/phase_audit/render_benchmark）A 无对应，排查噪域问题时缺少 B 侧工具。
5. **Reveal 缩放观感回归（中）**：`scale_zones_for_render` + `zone_on_screen` 是 A 特有路径（官方无全景揭晓），建议在 Reveal 模式下回归验证缩放观感、剔除正确性（判定已证明不受影响）。

---

## 7. 总体结论

- A 在**判定语义**上与 B 完全一致（block.rs 同源），官方 Phigros 噪域触摸失效规则被完整保留；
- A 的差异集中在：① fork 适配必需项（glCopy、音频 API、贴图路径）——均正确；② 性能/功能增强（zone 剔除、Reveal 缩放、显式断判）——不破坏官方语义；③ frag 三处计算简化——仅视觉细节，需回归确认；④ 预热与调试工具缺失——非功能缺陷，建议补齐。
*（内容由AI生成，仅供参考）*
