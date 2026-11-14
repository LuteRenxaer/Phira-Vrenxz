use anyhow::{bail, Context, Result};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricWord {
    pub text: String,
    pub start_time: f64,
    pub end_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LyricRole {
    Main,
    Duet,
    Background,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricLine {
    pub words: Vec<LyricWord>,
    pub start_time: f64,
    pub end_time: f64,
    pub role: LyricRole,
    pub agent: Option<String>,
}

pub type Lyrics = Vec<LyricLine>;

fn parse_time(s: &str) -> Result<f64> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => {
            // 纯秒格式（可带小数毫秒），如 "0.740"、"59.451"（Apple Music 风格 TTML 常用）
            let seconds: f64 = parts[0].parse()?;
            Ok(seconds)
        }
        2 => {
            let minutes: f64 = parts[0].parse()?;
            let seconds: f64 = parts[1].parse()?;
            Ok(minutes * 60. + seconds)
        }
        3 => {
            let hours: f64 = parts[0].parse()?;
            let minutes: f64 = parts[1].parse()?;
            let seconds: f64 = parts[2].parse()?;
            Ok(hours * 3600. + minutes * 60. + seconds)
        }
        _ => bail!("invalid time format: {}", s),
    }
}

/// 解码 XML 常见字符实体（顺序：&amp; 必须最后替换，避免双重转义）
fn decode_xml_entities(s: &str) -> String {
    s.replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn extract_attr<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    // 同时支持双引号与单引号属性（XML 两者均合法）
    for quote in ['"', '\''] {
        let prefix = format!("{}=\"", name);
        let prefix = if quote == '"' {
            prefix
        } else {
            prefix.replace("\"", "'")
        };
        if let Some(start) = s.find(&prefix) {
            let start = start + prefix.len();
            if let Some(end) = s[start..].find(quote) {
                return Some(&s[start..start + end]);
            }
        }
    }
    None
}

fn parse_ttml_span(s: &str, default_time: Option<(f64, f64)>) -> Result<LyricWord> {
    let text: String = {
        let content_start = s.find('>').context("span has no content")? + 1;
        let content_end = s[content_start..].find('<').context("span has no closing tag")?;
        decode_xml_entities(&s[content_start..content_start + content_end])
    };

    let start_time = extract_attr(s, "begin").and_then(|v| parse_time(v).ok()).or(default_time.map(|t| t.0));

    let end_time = extract_attr(s, "end").and_then(|v| parse_time(v).ok()).or(default_time.map(|t| t.1));

    if let (Some(start_time), Some(end_time)) = (start_time, end_time) {
        Ok(LyricWord {
            text,
            start_time,
            end_time,
        })
    } else {
        Err(anyhow::anyhow!("span has no time attributes and no default time provided"))
    }
}

fn parse_ttml_p(s: &str) -> Result<Vec<LyricLine>> {
    let p_start_time = extract_attr(s, "begin")
        .context("p has no begin attribute")
        .and_then(|v| parse_time(v))
        .context("invalid begin time")?;

    let p_end_time = extract_attr(s, "end")
        .context("p has no end attribute")
        .and_then(|v| parse_time(v))
        .context("invalid end time")?;

    let agent = extract_attr(s, "ttm:agent").map(|v| v.to_string());

    // 无 agent 属性的单轨 TTML 默认为主轨（与 LRC 单轨渲染在左侧一致）；
    // v1/main/1 视为主轨，其余（v2/duet/2 等）视为副轨
    let role = match agent.as_deref() {
        None | Some("v1") | Some("main") | Some("Main") | Some("1") => LyricRole::Main,
        _ => LyricRole::Duet,
    };

    let mut main_words = Vec::new();
    let mut bg_lines = Vec::new();

    let content_start = s.find('>').context("p has no content")? + 1;
    let content_end = s.rfind("</p>").context("p has no closing tag")?;
    let content = &s[content_start..content_end];
    let mut remaining = content;
    let mut in_bg = false;
    let mut bg_words = Vec::new();
    let mut bg_start_time = p_start_time;
    let mut bg_end_time = p_end_time;

    while let Some(start) = remaining.find("<span") {
        let span_str = &remaining[start..];
        let tag_end = span_str.find('>').context("span has no closing >")?;
        let tag_full = &span_str[..tag_end + 1];

        let is_bg = tag_full.contains(r#"ttm:role="x-bg""#);

        if is_bg {
            if in_bg && !bg_words.is_empty() {
                bg_lines.push(LyricLine {
                    words: bg_words,
                    start_time: bg_start_time,
                    end_time: bg_end_time,
                    role: LyricRole::Background,
                    agent: agent.clone(),
                });
            }

            in_bg = true;
            bg_words = Vec::new();

            bg_start_time = extract_attr(tag_full, "begin").and_then(|v| parse_time(v).ok()).unwrap_or(p_start_time);
            bg_end_time = extract_attr(tag_full, "end").and_then(|v| parse_time(v).ok()).unwrap_or(p_end_time);

            remaining = &remaining[start + tag_end + 1..];
        } else {
            let span_end = span_str.find("</span>").context("span has no closing tag")? + 7;
            let full_span = &span_str[..span_end];

            if in_bg {
                let default_time = bg_words
                    .last()
                    .map(|w: &LyricWord| (w.end_time, w.end_time))
                    .or(Some((bg_start_time, bg_end_time)));
                if let Ok(word) = parse_ttml_span(full_span, default_time) {
                    let mut clean_word = word;
                    clean_word.text = clean_word.text.replace(['(', ')'], "");
                    bg_words.push(clean_word);
                }

                let after_span = &remaining[start + span_end..];
                if after_span.starts_with("</span>") {
                    if !bg_words.is_empty() {
                        bg_lines.push(LyricLine {
                            words: bg_words,
                            start_time: bg_start_time,
                            end_time: bg_end_time,
                            role: LyricRole::Background,
                            agent: agent.clone(),
                        });
                    }
                    in_bg = false;
                    bg_words = Vec::new();
                    remaining = &after_span[7..];
                } else {
                    let trimmed = after_span.trim_start();
                    let has_space = after_span.len() != trimmed.len();
                    if has_space {
                        if let Some(end_time) = bg_words.last().map(|w| w.end_time) {
                            bg_words.push(LyricWord {
                                text: " ".to_string(),
                                start_time: end_time,
                                end_time: end_time,
                            });
                        }
                    }
                    remaining = trimmed;
                }
            } else {
                let default_time = main_words
                    .last()
                    .map(|w: &LyricWord| (w.end_time, w.end_time))
                    .or(Some((p_start_time, p_end_time)));
                if let Ok(word) = parse_ttml_span(full_span, default_time) {
                    main_words.push(word);
                }
                let after_span = &remaining[start + span_end..];
                let trimmed = after_span.trim_start();
                let has_space = after_span.len() != trimmed.len();
                if has_space {
                    if let Some(end_time) = main_words.last().map(|w| w.end_time) {
                        main_words.push(LyricWord {
                            text: " ".to_string(),
                            start_time: end_time,
                            end_time: end_time,
                        });
                    }
                }
                remaining = trimmed;
            }
        }
    }

    if in_bg && !bg_words.is_empty() {
        bg_lines.push(LyricLine {
            words: bg_words,
            start_time: bg_start_time,
            end_time: bg_end_time,
            role: LyricRole::Background,
            agent: agent.clone(),
        });
    }

    let mut result = Vec::new();
    if !main_words.is_empty() {
        let mut main_end_time = p_end_time;
        if !bg_lines.is_empty() {
            main_end_time = main_end_time.max(bg_lines[0].end_time);
        }
        result.push(LyricLine {
            words: main_words,
            start_time: p_start_time,
            end_time: main_end_time,
            role,
            agent,
        });
    }

    result.extend(bg_lines);
    Ok(result)
}

pub fn parse_ttml(source: &str) -> Result<Lyrics> {
    let mut lyrics = Vec::new();
    let mut remaining = source;

    while let Some(start) = remaining.find("<p") {
        // 仅接受 `<p` 后跟空白或 `>` 的标签（兼容属性换行写法）；
        // 无属性的 `<p>` 会在 parse_ttml_p 中因缺少 begin/end 被跳过，行为安全
        let after_p = remaining[start + 2..].chars().next();
        if !matches!(after_p, Some(' ') | Some('\t') | Some('\r') | Some('\n') | Some('>')) {
            remaining = &remaining[start + 2..];
            continue;
        }
        let p_end = remaining[start..].find("</p>").context("p has no closing tag")? + 4;
        let p_str = &remaining[start..start + p_end];
        if let Ok(lines) = parse_ttml_p(p_str) {
            lyrics.extend(lines);
        }
        remaining = &remaining[start + p_end..];
    }

    lyrics.sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap_or(std::cmp::Ordering::Equal));
    Ok(lyrics)
}

#[cfg(test)]
mod real_file_tests {
    use super::*;

    #[test]
    fn parse_real_chart_lyrics() {
        let cases = [
            r"F:\RustProject\phirLie\data\charts\custom\aac5bdbb-911a-4d95-8c45-c67f30b814e2\lyric.lrc",
            r"F:\RustProject\phirLie\data\charts\custom\e2748320-bebb-459e-829f-766708175498\lyric.ttml",
        ];
        for path in cases {
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path));
            match parse(&text) {
                Ok(lyrics) => {
                    println!("PARSED OK {} -> {} lines", path, lyrics.len());
                    for l in lyrics.iter().take(3) {
                        let joined: String = l.words.iter().map(|w| w.text.clone()).collect();
                        println!("  line start={:.3} end={:.3} role={:?} words={} text={:?}", l.start_time, l.end_time, l.role, l.words.len(), joined);
                    }
                }
                Err(e) => println!("PARSE FAILED {} -> {:#}", path, e),
            }
        }
    }
}

pub fn parse_lrc(source: &str) -> Result<Lyrics> {
    let mut lyrics = Vec::new();

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('[') && !line.contains(']') {
            continue;
        }

        let mut remaining = line;
        let mut times = Vec::new();
        let mut contents = Vec::new();

        while let Some(start) = remaining.find('[') {
            let end = remaining[start..].find(']').context("timestamp not closed")?;
            let timestamp = &remaining[start + 1..start + end];

            if let Ok(time) = parse_time(timestamp) {
                times.push(time);

                let content_start = start + end + 1;
                let content = if let Some(next_bracket) = remaining[content_start..].find('[') {
                    &remaining[content_start..content_start + next_bracket]
                } else {
                    &remaining[content_start..]
                };
                contents.push(content.to_string());
            }

            remaining = &remaining[start + end + 1..];
        }

        if times.is_empty() {
            continue;
        }

        let mut words = Vec::new();
        let line_start = times[0];

        if times.len() > 1 {
            for i in 0..times.len() {
                let start_time = times[i];
                let end_time = times.get(i + 1).copied().unwrap_or(start_time + 2.0);

                let content = &contents[i];
                let chars: Vec<char> = content.chars().collect();

                if chars.is_empty() {
                    continue;
                }

                for (j, c) in chars.iter().enumerate() {
                    let char_start = start_time + j as f64 * (end_time - start_time) / chars.len() as f64;
                    let char_end = if j == chars.len() - 1 {
                        end_time
                    } else {
                        char_start + (end_time - start_time) / chars.len() as f64
                    };

                    words.push(LyricWord {
                        text: c.to_string(),
                        start_time: char_start,
                        end_time: char_end,
                    });
                }
            }

            let line_end = times.last().copied().unwrap_or(line_start + 2.0);
            lyrics.push(LyricLine {
                words,
                start_time: line_start,
                end_time: line_end,
                role: LyricRole::Main,
                agent: None,
            });
        } else {
            let content = contents.join("").trim().to_string();

            if content.is_empty() {
                continue;
            }

            lyrics.push(LyricLine {
                words: vec![LyricWord {
                    text: content,
                    start_time: line_start,
                    end_time: line_start,
                }],
                start_time: line_start,
                end_time: line_start,
                role: LyricRole::Main,
                agent: None,
            });
        }
    }

    lyrics.sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap_or(std::cmp::Ordering::Equal));

    for i in 0..lyrics.len() - 1 {
        let next_start = lyrics[i + 1].start_time;

        if lyrics[i].end_time <= lyrics[i].start_time {
            lyrics[i].end_time = next_start;
            for word in &mut lyrics[i].words {
                word.end_time = next_start;
            }
        } else if lyrics[i].end_time > next_start {
            lyrics[i].end_time = next_start;
            for word in &mut lyrics[i].words {
                if word.end_time > next_start {
                    word.end_time = next_start;
                }
            }
        }
    }

    Ok(lyrics)
}

pub fn detect_format(source: &str) -> &'static str {
    // 剥离 UTF-8 BOM，避免带 BOM 的 XML/TTML 被误判
    let source = source.trim_start_matches('\u{feff}');
    if source.starts_with('<') {
        "ttml"
    } else if source.contains('[') && source.contains(']') {
        "lrc"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用真实逐词 LRC 样本验证 parse 产物，并模拟 update_lyrics 挤压 + render_lyrics
    /// 缓动时间线，定位上移失效根因（行间间隙导致 end_time >= start_time 永不成立）。
    #[test]
    fn real_lrc_parse_and_move_timeline() {
        // 用"自由落体"样本：行间有 0.3~0.4s 间隙（end_time < 下一行 start_time），
        // 旧实现挤压条件 end_time >= start_time 永不成立 -> 歌词直接消失、无上移。
        let path = r"F:\RustProject\phirLie\data\charts\custom\078dfc9a-f64a-440e-baa3-6acf2ff52b64\lyric.lrc";
        let bytes = std::fs::read(path).unwrap();
        let text = String::from_utf8_lossy(&bytes).to_string();
        let lyrics = parse(&text).unwrap();
        println!("parsed lines: {}", lyrics.len());
        for i in 0..lyrics.len().min(4) {
            let l = &lyrics[i];
            let first = l.words.first().map(|w| (w.text.clone(), w.start_time, w.end_time));
            let last = l.words.last().map(|w| (w.text.clone(), w.start_time, w.end_time));
            println!(
                "line[{}] start={:.2} end={:.2} role={:?} words={} first={:?} last={:?}",
                i, l.start_time, l.end_time, l.role, l.words.len(), first, last
            );
        }

        // ---- 模拟 update_lyrics 挤压逻辑（复制当前实现的关键字段流）----
        const FADE_IN_TIME: f64 = 0.5;
        const FADE_OUT_TIME: f64 = 0.3;
        const MOVE_TIME: f64 = 0.5;
        let text_height: f32 = 55.0; // 假设测量正常（55px，供仿真）

        // active 项: (start, end, disappear_start, y_offset_start, y_offset_target, move_start_time)
        let mut active: Vec<(f64, f64, f64, f32, f32, f64)> = Vec::new();
        let mut idx = 0usize;

        // 播放时间线采样：以行0(13.47)、行1(16.75)、行2(19.96) 为例，push 时刻 = start - 0.5
        let push_times = [12.97, 16.25, 19.46];
        // 直接逐步模拟：每步推进到 push 时刻执行 update，再渲染若干采样点
        for &t in &push_times {
            while idx < lyrics.len() && lyrics[idx].start_time <= t + FADE_IN_TIME {
                let line = &lyrics[idx];
                let y_offset_start = 0.0f32;
                let y_offset_target = 0.0f32;
                let move_start_time = t;
                if !matches!(line.role, LyricRole::Background) {
                    let nfw = line.words.first().map(|w| w.start_time).unwrap_or(line.start_time);
                    let next_move_start_time = nfw - 0.05;
                    for a in &mut active {
                        // 复刻当前挤压条件：主轨新句挤压"还在显示中的旧主轨句"，
                        // 不依赖时间重叠（行间有间隙也能触发上移）。
                        // 元组模拟未存 role，样本 LRC 全为 Main 角色，故省略 role 判断
                        let should_move = match line.role {
                            LyricRole::Main => a.2 + FADE_OUT_TIME > t,
                            LyricRole::Duet => a.1 >= line.start_time,
                            LyricRole::Background => false,
                        };
                        if should_move {
                            a.3 = a.4;
                            a.4 -= text_height * 1.5;
                            a.5 = (a.0 + 1.2).max(next_move_start_time);
                            // 修复后：被挤压旧句延后淡出，上移动画完成后停留 0.5s 再淡出
                            a.2 = (a.5 + MOVE_TIME + 0.5).max(a.2);
                        }
                    }
                }
                let disappear_start = line.words.last().map(|w| w.end_time).unwrap_or(line.end_time);
                active.push((line.start_time, line.end_time, disappear_start, y_offset_start, y_offset_target, move_start_time));
                idx += 1;
            }
        }

        // 渲染采样：行0 的 move 区间 [16.70, 17.20]；修复前无挤压时 cur 恒为 0
        let a0 = &active[0];
        println!("\nactive[0]: start={:.2} end={:.2} disappear_start={:.2} y_off_s={} y_off_t={} move_start={:.2}",
            a0.0, a0.1, a0.2, a0.3, a0.4, a0.5);
        println!("采样: t | move_progress | eased | current_y_offset | disappear_progress | alpha");
        let mut s = 16.55;
        while s <= 18.10 {
            let mp = ((s - a0.5) / MOVE_TIME).clamp(0.0, 1.0);
            let eased = 1.0 - (1.0 - mp).powi(4);
            let cur = a0.3 + (a0.4 - a0.3) * eased as f32;
            let dp = ((s - a0.2) / FADE_OUT_TIME).clamp(0.0, 1.0);
            let appear_p = ((s - a0.0) / FADE_IN_TIME).clamp(0.0, 1.0);
            let alpha = (1.0 - dp) * appear_p;
            println!("{:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2}", s, mp, eased, cur, dp, alpha);
            s += 0.05;
        }
    }
}

pub fn parse(source: &str) -> Result<Lyrics> {
    match detect_format(source) {
        "ttml" => parse_ttml(source),
        "lrc" => parse_lrc(source),
        _ => bail!("unknown lyrics format"),
    }
}
