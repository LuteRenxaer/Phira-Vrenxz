//! 集成测试：验证 Apple Music 风格 TTML 歌词能被正确解析为 LyricLine/LyricWord。
//!
//! 样本文件：tests/data/lyric.ttml（Counting Stars，95 段，v1 主轨 / v2 副轨，
//! 时间格式混合纯秒（0.740）与 mm:ss.xxx（1:02.873），文本含 &apos; 实体）。

use prpr::parse::lyrics::{LyricRole, parse};

fn load_sample() -> String {
    std::fs::read_to_string("tests/data/lyric.ttml").expect("读取测试样本失败")
}

#[test]
fn ttml_parse_basic() {
    let lyrics = parse(&load_sample()).expect("TTML 解析失败");
    assert_eq!(lyrics.len(), 95, "应有 95 段歌词");

    // v1 主轨 63 段，v2 副轨 32 段（95 = 63 + 32）
    let main_count = lyrics.iter().filter(|l| matches!(l.role, LyricRole::Main)).count();
    let duet_count = lyrics.iter().filter(|l| matches!(l.role, LyricRole::Duet)).count();
    assert_eq!(main_count, 63);
    assert_eq!(duet_count, 32);
}

#[test]
fn ttml_parse_first_line() {
    let lyrics = parse(&load_sample()).expect("TTML 解析失败");
    let first = &lyrics[0];

    assert!(matches!(first.role, LyricRole::Main));
    assert_eq!(first.agent.as_deref(), Some("v1"));
    assert!((first.start_time - 0.740).abs() < 1e-6, "start_time 应为 0.740, 实际 {}", first.start_time);
    assert!((first.end_time - 5.558).abs() < 1e-6, "end_time 应为 5.558, 实际 {}", first.end_time);

    // 首词：Late @ 0.740 ~ 1.350（不含词间空格词，首个词应为纯文本词）
    let first_word = &first.words[0];
    assert_eq!(first_word.text, "Late");
    assert!((first_word.start_time - 0.740).abs() < 1e-6);
    assert!((first_word.end_time - 1.350).abs() < 1e-6);

    // 实体解码：第 3 个纯文本词为 I've（原文件为 I&apos;ve）
    let texts: Vec<&str> = first.words.iter().filter(|w| !w.text.trim().is_empty()).map(|w| w.text.as_str()).collect();
    assert_eq!(texts[2], "I've", "&apos; 实体应解码为撇号");

    // 9 个文本词 + 6 个空格词（Late-ly, 与 los-ing 两处间隔无空格，紧贴不分词）
    assert_eq!(first.words.len(), 15);
    assert_eq!(first.words.iter().filter(|w| w.text == " ").count(), 6);
}

#[test]
fn ttml_parse_pure_seconds_and_mmss() {
    let lyrics = parse(&load_sample()).expect("TTML 解析失败");

    // L7 纯秒时间（38.051 秒处）
    let l7 = lyrics.iter().find(|l| l.agent.as_deref() == Some("v1") && (l.start_time - 38.051).abs() < 1e-6);
    assert!(l7.is_some(), "纯秒时间 38.051 应能被解析");

    // L16 混合格式：begin="59.451" end="1:02.873" → 59.451 / 62.873
    let l16 = lyrics.iter().find(|l| (l.start_time - 59.451).abs() < 1e-6);
    assert!(l16.is_some(), "59.451 应能被解析");
    if let Some(l) = l16 {
        assert!((l.end_time - 62.873).abs() < 1e-6, "1:02.873 应为 62.873 秒, 实际 {}", l.end_time);
    }
}

#[test]
fn ttml_parse_last_line() {
    let lyrics = parse(&load_sample()).expect("TTML 解析失败");
    let last = lyrics.last().unwrap();

    // 最后一段 L95：begin="4:13.007" end="4:15.439" → 253.007 / 255.439
    assert!(matches!(last.role, LyricRole::Duet));
    assert_eq!(last.agent.as_deref(), Some("v2"));
    assert!((last.start_time - 253.007).abs() < 1e-6);
    assert!((last.end_time - 255.439).abs() < 1e-6);
    assert_eq!(last.words[0].text, "The");
}
