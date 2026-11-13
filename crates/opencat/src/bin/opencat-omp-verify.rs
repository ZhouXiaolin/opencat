//! `@omp` 对拍台——rayon 原生实现的 go/no-go 门。
//!
//! 读 V8 采样探针（OC_OMP_VERIFY=1，html.rs driver 注入）dump 到 stderr 的
//! `[omp:<fn>]` 行，用 `libm` crate（musl fdlibm 血统，与 V8 的
//! ieee754.cc 同源）同输入复算，逐位对比。sin/cos/exp/hypot 决定 halftone
//! 点的阈值分支（有无/半径/颜色索引），必须逐位一致；toFixed(3) 只进
//! alpha 的 1/255 级颜色，p8 判据下无害，对拍确认差异面即可。
//!
//! 行格式（html.rs 探针 dump）：
//! - `[omp:sin] <x> <xbits> <outbits>`（cos/exp 同）
//! - `[omp:hypot] <x> <y> <xbits> <ybits> <outbits>`
//! - `[omp:fix] <x> <js-string>`
//!
//! 用法：`opencat-omp-verify <stderr-log>`

use std::io::Write;

/// 探针的位型格式 `409f1234_56789abc`；非有限值 dump 成名字。
fn parse_bits(s: &str) -> Option<u64> {
    let (hi, lo) = s.split_once('_')?;
    Some((u64::from_str_radix(hi, 16).ok()? << 32) | u64::from_str_radix(lo, 16).ok()?)
}

fn ulp_gap(a: u64, b: u64) -> u64 {
    (a as i64 - b as i64).unsigned_abs()
}

#[derive(Default)]
struct FnStat {
    ok: u64,
    bad: u64,
    nonfinite: u64,
    max_ulp: u64,
    worst: Option<(f64, u64, u64)>, // (输入, JS 位型, libm 位型)
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("usage: opencat-omp-verify <stderr-log>");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("read {path}: {e}"))?;

    let mut stats: Vec<(&str, FnStat)> = vec![
        ("sin", FnStat::default()),
        ("cos", FnStat::default()),
        ("exp", FnStat::default()),
        ("hypot", FnStat::default()),
        ("fix", FnStat::default()),
    ];

    for line in text.lines() {
        // stderr_log 给每行加 `[js] ` 前缀，定位 `[omp:` 子串再解析。
        let Some(pos) = line.find("[omp:") else { continue };
        let line = &line[pos..];
        let Some((tag, rest)) = line.split_once("] ") else { continue };
        let Some(tag) = tag.strip_prefix("[omp:") else { continue };
        let mut it = rest.split_whitespace();
        let Some(slot) = stats.iter_mut().find(|(n, _)| *n == tag) else { continue };
        let s = &mut slot.1;
        match tag {
            "sin" | "cos" | "exp" => {
                let x_txt = it.next();
                let in_bits_s = it.next();
                let out_bits_s = it.next();
                let Some(in_bits) = in_bits_s.and_then(parse_bits) else { continue };
                let Some(out_bits) = out_bits_s.and_then(parse_bits) else {
                    s.nonfinite += 1;
                    continue;
                };
                let x = f64::from_bits(in_bits);
                let _ = x_txt; // 十进制串仅供人读，位型是真值
                let got = match tag {
                    "sin" => libm::sin(x),
                    "cos" => libm::cos(x),
                    _ => libm::exp(x),
                };
                record(s, got.to_bits(), out_bits, x);
            }
            "hypot" => {
                let _x_txt = it.next();
                let _y_txt = it.next();
                let x_bits_s = it.next();
                let _y_bits_s = it.next();
                let out_bits_s = it.next();
                // 页面调用是双参 hypot；对拍面只需入参位型可复算——探针
                // dump 的 ybits 同样是真值，但行解析只取 x 做单参复算不
                // 足以对拍。这里取 xbits 作 x、ybits 作 y 完整复算。
                let xb = x_bits_s.and_then(parse_bits);
                let yb = _y_bits_s.and_then(parse_bits);
                let Some(out_bits) = out_bits_s.and_then(parse_bits) else {
                    s.nonfinite += 1;
                    continue;
                };
                let (Some(xb), Some(yb)) = (xb, yb) else { continue };
                let got = libm::hypot(f64::from_bits(xb), f64::from_bits(yb));
                record(s, got.to_bits(), out_bits, f64::from_bits(xb));
            }
            "fix" => {
                let Some((x_txt, js_txt)) = rest.split_once(' ') else { continue };
                let Ok(x) = x_txt.parse::<f64>() else { continue };
                // V8 的 toFixed 用 double-conversion（正确舍入）；Rust fmt
                // 同为正确舍入，ties 规则差异只可能出现在精确 .0005 边界。
                let rs = format!("{x:.3}");
                if rs == js_txt {
                    s.ok += 1;
                } else {
                    s.bad += 1;
                    if s.worst.is_none() {
                        s.worst = Some((x, 0, 0));
                    }
                }
            }
            _ => {}
        }
    }

    let mut out = std::io::stdout().lock();
    writeln!(out, "{:<8} {:>8} {:>8} {:>10} {:>8}", "fn", "ok", "bad", "nonfinite", "max_ulp")?;
    for (name, s) in &stats {
        writeln!(out, "{:<8} {:>8} {:>8} {:>10} {:>8}", name, s.ok, s.bad, s.nonfinite, s.max_ulp)?;
        if let Some((x, js, got)) = s.worst {
            if *name == "fix" {
                writeln!(out, "  first-mismatch: x={x}")?;
            } else {
                writeln!(out, "  worst: x={x:e} js={js:#018x} libm={got:#018x}")?;
            }
        }
    }
    // fix 只进 alpha（1/255 级），允许差异；其余函数必须逐位一致。
    let all_ok = stats.iter().all(|(n, s)| n == &"fix" || s.bad == 0);
    writeln!(out, "\n{}", if all_ok { "GO — 超越函数逐位一致" } else { "NO-GO — 存在位型差异，评估翻转率" })?;
    Ok(())
}

fn record(s: &mut FnStat, got: u64, js: u64, x: f64) {
    if got == js {
        s.ok += 1;
    } else {
        s.bad += 1;
        let gap = ulp_gap(got, js);
        if gap > s.max_ulp {
            s.max_ulp = gap;
            s.worst = Some((x, js, got));
        }
    }
}
