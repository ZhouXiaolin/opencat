//! Lambda 编译错误：消息 + lambda 源码内位置（行列）+ 源码摘录。

use std::fmt;

#[derive(Debug, Clone)]
pub struct LambdaError {
    pub message: String,
    /// lambda 源码内的字节偏移区间。
    pub span: Option<(u32, u32)>,
    pub source: String,
}

impl LambdaError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self { message: message.into(), span: None, source: String::new() }
    }

    pub fn at(message: impl Into<String>, span: (u32, u32), source: &str) -> Self {
        Self { message: message.into(), span: Some(span), source: source.to_string() }
    }

    /// 计算 1-based 行列（span 起点）。
    pub fn line_col(&self) -> Option<(usize, usize)> {
        let (start, _) = self.span?;
        let mut line = 1usize;
        let mut col = 1usize;
        for b in self.source.bytes().take(start as usize) {
            if b == b'\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        Some((line, col))
    }

    fn excerpt(&self) -> Option<String> {
        let (start, _) = self.span?;
        let (_, col) = self.line_col()?;
        let line_start = self.source[..start as usize].rfind('\n').map_or(0, |i| i + 1);
        let line_end = self.source[start as usize..]
            .find('\n')
            .map_or(self.source.len(), |i| start as usize + i);
        let text = &self.source[line_start..line_end];
        let caret = " ".repeat((col - 1).min(text.len()));
        Some(format!("  {text}\n  {caret}^"))
    }
}

impl fmt::Display for LambdaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "effect lambda: {}", self.message)?;
        if let Some((line, col)) = self.line_col() {
            write!(f, "（lambda 源码第 {line} 行第 {col} 列）")?;
            if let Some(excerpt) = self.excerpt() {
                write!(f, "\n{excerpt}")?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for LambdaError {}

pub type LambdaResult<T> = Result<T, LambdaError>;
