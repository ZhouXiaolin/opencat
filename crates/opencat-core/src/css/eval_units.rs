//! CSS 文本值的单位换算（脚本写入的标准 CSS 属性通道）。

/// 字体相对长度按元素计算字号折成 px。
///
/// `em` 相对本元素字号；`rem` 相对根字号（合成帧固定 16px）。视口单位按
/// 合成画布尺寸，查询容器单位按最近尺寸查询容器，回退到视口。
pub fn to_px(
    text: &str,
    font_px: f32,
    viewport: (f32, f32),
    query_container: Option<(f32, f32)>,
) -> Option<f32> {
    let text = text.trim();
    let (number, unit) = split_number_unit(text);
    let value = number.parse::<f32>().ok()?;
    let query = query_container.unwrap_or(viewport);
    Some(match unit {
        "" | "px" => value,
        "em" => value * font_px,
        "rem" => value * 16.0,
        "vw" => value * viewport.0 / 100.0,
        "vh" => value * viewport.1 / 100.0,
        "vmin" => value * viewport.0.min(viewport.1) / 100.0,
        "vmax" => value * viewport.0.max(viewport.1) / 100.0,
        "cqw" => value * query.0 / 100.0,
        "cqh" => value * query.1 / 100.0,
        "pt" => value * 96.0 / 72.0,
        "pc" => value * 16.0,
        "in" => value * 96.0,
        "cm" => value * 96.0 / 2.54,
        "mm" => value * 96.0 / 25.4,
        _ => return None,
    })
}

fn split_number_unit(text: &str) -> (&str, &str) {
    let end = text
        .bytes()
        .position(|byte| !matches!(byte, b'0'..=b'9' | b'+' | b'-' | b'.' | b'e' | b'E'))
        .unwrap_or(text.len());
    (&text[..end], &text[end..])
}
