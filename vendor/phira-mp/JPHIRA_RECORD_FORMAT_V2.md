# Phira 录制文件格式规范 (V2)

> 文件扩展名: `.phirarec`  
> 格式版本: 1 (V2 格式)  
> 字节序: 小端序 (Little Endian)

---

## 1. 文件格式概述

Phira 录制文件使用二进制格式存储，主要由 **文件头**、**格式版本**、**压缩类型** 和 **压缩数据** 四部分组成。

---

## 2. 文件结构

```
┌─────────────────────────────────────────────────────────────────────┐
│                         JPhiraRec 格式                              │
├─────────────────────────────────────────────────────────────────────┤
│  偏移量    │   大小    │                字段                        │
├────────────┼───────────┼────────────────────────────────────────────┤
│  0x00      │   8 bytes │  文件头 Magic: "PHIRAREC"                  │
│  0x08      │   4 bytes │  格式版本: 1 (int32)                       │
│  0x0C      │   1 byte  │  压缩类型 (见下文压缩类型表)                │
│  0x0D      │   N bytes │  压缩后的数据块                            │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. 详细字段说明

### 3.1 文件头 (File Header)

| 字段 | 大小 | 值 | 说明 |
|------|------|-----|------|
| Magic | 8 bytes | `PHIRAREC` | ASCII 字符串，用于识别文件类型 |

### 3.2 格式版本 (Format Version)

| 字段 | 大小 | 值 | 说明 |
|------|------|-----|------|
| Version | 4 bytes (int32) | `1` | 当前 V2 格式的版本号为 1 |

### 3.3 压缩类型 (Compression Type)

| ID | 类型 | 说明 |
|----|------|------|
| `0x00` | NONE | 无压缩 |
| `0x01` | ZSTD | Zstandard 压缩 (默认) |
| `0x02` | DEFLATE | DEFLATE 压缩 |

### 3.4 压缩数据块结构 (解压后)

```
┌─────────────────────────────────────────────────────────────────────┐
│                      录制数据 (解压后)                               │
├─────────────────────────────────────────────────────────────────────┤
│  偏移量    │   大小    │                字段                        │
├────────────┼───────────┼────────────────────────────────────────────┤
│  0x00      │   4 bytes │  录制记录 ID (int32)                       │
│  0x04      │   8 bytes │  录制时间戳 (int64, Unix 时间戳毫秒)       │
│  0x0C      │   4 bytes │  谱面 ID (int32)                           │
│  0x10      │   变长    │  谱面名称 (字符串)                         │
│            │           │    - 1-4 bytes: 字符串长度 (varint32 leb128) │
│            │           │    - N bytes: UTF-8 编码的字符串内容       │
│  变长      │   4 bytes │  用户 ID (int32)                           │
│  变长      │   变长    │  用户名称 (字符串，编码同上)               │
│  变长      │   变长    │  触摸帧列表 (TouchFrame List)              │
│  变长      │   变长    │  判定事件列表 (JudgeEvent List)            │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 4. 数据类型定义

### 4.1 字符串编码

字符串使用以下格式编码：
- **长度**: 1-4 bytes (varint32 leb128) - 字符串字节长度
- **内容**: N bytes - UTF-8 编码的字符串

### 4.2 列表编码

列表使用以下格式编码：
- **长度**: 1-4 bytes (varint32 leb128) - 列表元素个数
- **元素**: 变长 - 使用对应类型的 decode 方法解码的连续元素

### 4.3 浮点数编码

- **float (32位)**: 4 bytes，IEEE 754 单精度浮点数，小端序
- **float16 (16位)**: 2 bytes，半精度浮点数编码，小端序

---

## 5. 触摸帧数据结构 (TouchFrame)

### 5.1 TouchFrame

```java
@Getter
@RequiredArgsConstructor
public class TouchFrame implements Encodeable {

    private final float time;           // 时间戳（秒）
    private final List<TouchPoint> points;  // 触摸点列表

    public static TouchFrame decode(ByteBuf buf) {
        return new TouchFrame(
                buf.readFloatLE(),
                NettyPacketUtil.decodeList(buf, TouchPoint::decode)
        );
    }

    @Override
    public void encode(ByteBuf buf) {
        PacketWriter.write(buf, time);
        PacketWriter.write(buf, points);
    }
}
```

**二进制格式：**

| 字段 | 大小 | 类型 | 说明 |
|------|------|------|------|
| time | 4 bytes | float32 (LE) | 帧时间戳（秒） |
| points | 变长 | List<TouchPoint> | 触摸点列表 |

---

### 5.2 TouchPoint

```java
@Getter
@RequiredArgsConstructor
public class TouchPoint implements Encodeable {

    private final byte id;              // 触摸点ID
    private final CompactPos pos;       // 位置信息

    public static TouchPoint decode(ByteBuf buf) {
        return new TouchPoint(
                buf.readByte(),
                CompactPos.decode(buf)
        );
    }

    @Override
    public void encode(ByteBuf buf) {
        PacketWriter.write(buf, id);
        PacketWriter.write(buf, pos);
    }
}
```

**二进制格式：**

| 字段 | 大小 | 类型 | 说明 |
|------|------|------|------|
| id | 1 byte | int8 | 触摸点标识符 |
| pos | 4 bytes | CompactPos | 坐标位置 |

---

### 5.3 CompactPos

```java
@Getter
@RequiredArgsConstructor
public class CompactPos implements Encodeable {

    private final float x;              // X坐标 (0-1范围)
    private final float y;              // Y坐标 (0-1范围)

    public static CompactPos decode(ByteBuf buf) {
        return new CompactPos(
                NettyPacketUtil.decodeFloat16LE(buf),
                NettyPacketUtil.decodeFloat16LE(buf)
        );
    }

    @Override
    public void encode(ByteBuf buf) {
        PacketWriter.writeFloat16(buf, x);
        PacketWriter.writeFloat16(buf, y);
    }
}
```

**二进制格式：**

| 字段 | 大小 | 类型 | 说明 |
|------|------|------|------|
| x | 2 bytes | float16 (LE) | X坐标，范围 0.0 ~ 1.0 |
| y | 2 bytes | float16 (LE) | Y坐标，范围 0.0 ~ 1.0 |

**说明：**
- `CompactPos` 使用 16位半精度浮点数存储坐标

---

## 6. 判定事件数据结构 (JudgeEvent)

### 6.1 JudgeEvent

```java
@Getter
@RequiredArgsConstructor
public class JudgeEvent implements Encodeable {

    private final float time;           // 判定时间（秒）
    private final int lineId;           // 判定线ID
    private final int noteId;           // 音符ID
    private final Judgement judgement;  // 判定结果

    public static JudgeEvent decode(ByteBuf buf) {
        return new JudgeEvent(
                buf.readFloatLE(),
                buf.readIntLE(),
                buf.readIntLE(),
                Judgement.decode(buf)
        );
    }

    @Override
    public void encode(ByteBuf buf) {
        PacketWriter.write(buf, time);
        PacketWriter.write(buf, lineId);
        PacketWriter.write(buf, noteId);
        PacketWriter.write(buf, judgement);
    }
}
```

**二进制格式：**

| 字段 | 大小 | 类型 | 说明 |
|------|------|------|------|
| time | 4 bytes | float32 (LE) | 判定时间戳（秒） |
| lineId | 4 bytes | int32 (LE) | 判定线标识符 |
| noteId | 4 bytes | int32 (LE) | 音符标识符 |
| judgement | 1 byte | Judgement | 判定结果枚举值 |

---

### 6.2 Judgement (判定类型枚举)

```java
@RequiredArgsConstructor
@Getter(AccessLevel.PRIVATE)
public enum Judgement implements Encodeable {
    Perfect(0x00),
    Good(0x01),
    Bad(0x02),
    Miss(0x03),
    HoldPerfect(0x04),
    HoldGood(0x05);

    private final int id;

    private static Map<Integer,Judgement> getJudgementMap() {
        return Map.copyOf(Arrays.stream(values()).collect(Collectors.toMap(
                Judgement::getId,
                Function.identity()
        )));
    }

    private static final Map<Integer,Judgement> JUDGEMENT_MAP = getJudgementMap();

    public static Judgement decode(ByteBuf buf) {
        int id = buf.readByte();
        Judgement judgement = JUDGEMENT_MAP.get(id);
        if (judgement == null) {
            throw new DecoderException("Unknown Judgement id: " + id);
        }
        return judgement;
    }

    @Override
    public void encode(ByteBuf buf) {
        PacketWriter.writeByte(buf, id);
    }
}
```

**判定类型表：**

| ID | 名称 | 说明 |
|----|------|------|
| `0x00` | Perfect | 完美判定 |
| `0x01` | Good | 良好判定 |
| `0x02` | Bad | 较差判定 |
| `0x03` | Miss | 失误判定 |
| `0x04` | HoldPerfect | 长按完美判定 |
| `0x05` | HoldGood | 长按良好判定 |

---

## 7. 完整数据结构层次

```
PhiraRecord (录制文件)
├── File Header (8 bytes): "PHIRAREC"
├── Format Version (4 bytes): 1
├── Compression Type (1 byte)
└── Compressed Data
    └── Decompressed Content
        ├── Record ID (4 bytes)
        ├── Timestamp (8 bytes)
        ├── Chart ID (4 bytes)
        ├── Chart Name (string)
        ├── User ID (4 bytes)
        ├── User Name (string)
        ├── TouchFrame[]
        │   ├── Time (4 bytes float)
        │   └── TouchPoint[]
        │       ├── ID (1 byte)
        │       └── CompactPos
        │           ├── X (2 bytes float16)
        │           └── Y (2 bytes float16)
        └── JudgeEvent[]
            ├── Time (4 bytes float)
            ├── Line ID (4 bytes)
            ├── Note ID (4 bytes)
            └── Judgement (1 byte)
```

---

## 8. 其他支持的格式

### 8.1 TPhiraRec 格式 (旧版)

```
┌─────────────────────────────────────────────────────────────────────┐
│                        TPhiraRec 格式                               │
├─────────────────────────────────────────────────────────────────────┤
│  偏移量    │   大小    │                字段                        │
├────────────┼───────────┼────────────────────────────────────────────┤
│  0x00      │   2 bytes │  Magic: 0x504d 或 0x4d50 (short)          │
│  0x02      │   4 bytes │  谱面 ID (int32)                           │
│  0x06      │   4 bytes │  用户 ID (int32)                           │
│  0x0A      │   4 bytes │  记录 ID (int32)                           │
│  0x0E      │   N bytes │  数据包序列 (变长)                         │
└─────────────────────────────────────────────────────────────────────┘
```

**数据包序列格式：**
```
┌─────────────────────────────────────────┐
│  4 bytes   │  N bytes  │  数据包长度    │
│  N bytes   │           │  数据包内容    │
└─────────────────────────────────────────┘
```

每个数据包使用 `PacketRegistry.ServerBound.decode()` 解码。

---

## 9. 版本历史

| 版本 | 说明 |
|------|------|
| V1 (fileVersion=0) | 早期版本，无压缩，时间戳从记录 ID 获取 |
| V2 (fileVersion=1) | 当前版本，支持多种压缩算法，直接存储时间戳 |

---

## 10. 代码示例

### 10.1 读取录制文件

```java
// 从文件读取
PhiraRecord record = PhiraRecord.readFromFile(Path.of("12345.phirarec"));

// 从目录批量读取
List<PhiraRecord> records = PhiraRecord.readFromDirectory(Path.of("./records"));
```

### 10.2 创建录制文件

```java
PhiraRecord record = new PhiraRecord(
    12345,                          // 记录 ID
    System.currentTimeMillis(),     // 时间戳
    100,                            // 谱面 ID
    "TestChart",                    // 谱面名称
    1,                              // 用户 ID
    "Player1",                      // 用户名称
    touchFrames,                    // 触摸帧列表
    judgeEvents                     // 判定事件列表
);

// 保存到文件
PhiraRecord.saveAsFile(record, Path.of("./records"));
```

---

## 11. 默认配置

| 配置项 | 默认值 |
|--------|--------|
| 压缩类型 | ZSTD |
| 压缩级别 | Zstd.defaultCompressionLevel() |

---

## 12. 相关依赖

```groovy
implementation 'com.github.lRENyaaa:jphira-mp-protocol:2.2.1'
implementation 'com.github.luben:zstd-jni:1.5.6-6'
```
