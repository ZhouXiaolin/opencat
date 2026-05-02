# 字幕转录指南

## 字幕如何生成

OpenCat 没有内置的转录 CLI。你需要使用外部工具生成 SRT 字幕文件，然后由 `type: "caption"` 节点读取。

推荐的外置转录工具：

- **whisper.cpp** — 本地运行，无需 API Key
- **OpenAI Whisper API** — 云端，质量最佳
- **Groq Whisper API** — 速度快，有免费额度
- 任何能输出 SRT/JSON 的工具

## 使用 whisper.cpp（本地）

```bash
# 基础转录（使用默认模型）
whisper.cpp/main -f audio.mp3 -osrt -of transcript

# 指定模型
whisper.cpp/main -f audio.mp3 -m whisper.cpp/models/ggml-medium.bin -osrt -of transcript

# 限制为英语
whisper.cpp/main -f audio.mp3 -m whisper.cpp/models/ggml-small.en.bin -osrt -of transcript --language en
```

输出为 `transcript.srt`，可直接用于 OpenCat 的 caption 节点。

## 支持的输入格式

OpenCat 的 `type: "caption"` 节点读取 **SRT 字幕文件**。以下是可从外部工具获取的格式及转换方式：

| 格式                | 扩展名   | 来源                                  | 词级时间戳      |
| ------------------- | -------- | ------------------------------------- | --------------- |
| whisper.cpp JSON    | `.json`  | whisper.cpp 输出                      | 是              |
| OpenAI Whisper API  | `.json`  | `openai.audio.transcriptions.create`  | 是              |
| SRT 字幕            | `.srt`   | 视频编辑器、字幕工具、YouTube         | 否（短语级）    |
| VTT 字幕            | `.vtt`   | 网页播放器、YouTube、转录服务         | 否（短语级）    |

**词级时间戳可以生成更好的字幕效果。** SRT 是短语级时间，能正常工作但无法实现逐词动画。

所有格式最终都需转换为 SRT 供 OpenCat 使用。JSON 格式可使用 `references/transcript-to-srt.js` 脚本转换。

## Whisper 模型指南

默认模型（`small.en`）在精度和速度之间取得平衡。如需更好效果，使用更大的模型：

| 模型        | 大小    | 速度    | 准确度  | 使用场景                            |
| ----------- | ------- | ------- | ------- | ----------------------------------- |
| `tiny`      | 75 MB   | 最快    | 低      | 快速预览、测试流程                  |
| `base`      | 142 MB  | 快      | 一般    | 短片段、清晰的音频                  |
| `small`     | 466 MB  | 中等    | 好      | **默认** — 适用于大多数内容         |
| `medium`    | 1.5 GB  | 慢      | 很好    | 重要内容、嘈杂音频、音乐            |
| `large-v3`  | 3.1 GB  | 最慢    | 最佳    | 生产质量                            |

**仅当用户明确说明音频是英语时才添加 `.en` 后缀。** `.en` 模型对英语略微更准确，但会将非英语音频**翻译**成英语而非转录。

**关键：`.en` 模型将非英语音频翻译成英语** — 而不是转录它。如果音频可能不是英语，始终使用不带 `.en` 后缀的模型，并通过 `--language` 指定源语言。如果不确定语言，使用 `small`（非 `small.en`）且不加 `--language` — whisper 会自动检测。

```bash
# 西班牙语音频
whisper.cpp/main -f audio.mp3 -m whisper.cpp/models/ggml-small.bin -osrt -of transcript --language es

# 未知语言 — 让 whisper 自动检测
whisper.cpp/main -f audio.mp3 -m whisper.cpp/models/ggml-small.bin -osrt -of transcript
```

**音乐和人声轨道**：`small.en` 会错误识别歌词 — 至少使用 `medium.en`，或手动导入歌词。即使 `medium.en` 在制作精良的音频上也会出问题；对于音乐视频，提供已知歌词作为 SRT 文件始终优于自动转录。

## 字幕质量检查（必做）

每次转录后，**阅读字幕并检查质量后再继续。** 糟糕的字幕会产生无意义的 caption 效果。切勿跳过此步骤。

### 检查内容

| 信号                       | 示例                                    | 原因                                      |
| -------------------------- | --------------------------------------- | ----------------------------------------- |
| 音符标记（`♪`、`�`）       | `♪` 或 `�`                              | Whisper 检测到音乐而非语音                |
| 乱码/无意义词语            | 各种乱码                                | 模型误听歌词或背景噪音                    |
| 长时间无词语的空白段        | 20+ 秒仅有 `♪` 标记                     | 乐器段落 — 可接受，但占比高说明漏语音     |
| 重复填充词                 | 大量 "huh"、"uh"、"oh"                  | 模型在音乐上产生幻觉                      |
| 极短的词时间跨度           | `end - start < 0.05` 的词               | 时间戳对齐不可靠                          |

### 自动重试规则

**如果超过 20% 的条目是 `♪`/`�` 标记，或字幕包含明显的无意义词语，转录失败。** 不要使用有问题的字幕。改为：

1. **使用 `medium.en` 重试**（如果原版使用了 `small.en` 或更小）：
   ```bash
   whisper.cpp/main -f audio.mp3 -m whisper.cpp/models/ggml-medium.bin -osrt -of transcript
   ```
2. **如果 `medium.en` 也失败**（仍 >20% 音乐标记或乱码），告知用户音频噪音太大，本地转录无法处理，建议：
   - 手动提供歌词作为 SRT 文件
   - 使用外部 API（OpenAI 或 Groq Whisper — 见下文）
3. **始终在构建 caption 前清理字幕** — 过滤掉 `♪`/`�` 标记以及单个非词字符的条目。只有真正的词语应进入 caption 合成。

### 清理字幕

转录后（即使使用好的模型），去除非词条目：

```js
var raw = JSON.parse(transcriptJson);
var words = raw.filter(function (w) {
  if (!w.text || w.text.trim().length === 0) return false;
  if (/^[♪�\u266a\u266b\u266c\u266d\u266e\u266f]+$/.test(w.text)) return false;
  if (/^(huh|uh|um|ah|oh)$/i.test(w.text) && w.end - w.start < 0.1) return false;
  return true;
});
```

清理后，将 JSON 转换为 SRT 供 OpenCat 使用。

### 何时使用哪个模型（决策树）

1. **这是静音/浅背景上的语音？** → `small.en` 即可
2. **这是音乐上的语音，或带人声的音乐？** → 从 `medium.en` 开始
3. **这是制作精良的音乐（人声 + 完整伴奏）？** → 从 `medium.en` 开始，预期需要手动歌词或外部 API
4. **这是多语言内容？** → 使用 `medium` 或 `large-v3`（无 `.en` 后缀）

## 使用外部转录 API

为获得最佳精度，使用外部 API 并将结果转为 SRT：

**OpenAI Whisper API**（推荐，质量最佳）：

```bash
# 生成带词级时间戳的转录
curl https://api.openai.com/v1/audio/transcriptions \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -F file=@audio.mp3 -F model=whisper-1 \
  -F response_format=verbose_json \
  -F "timestamp_granularities[]=word" \
  -o transcript-openai.json

# 将 JSON 转换为 SRT（使用转换脚本）
# 然后使用生成的 transcript.srt
```

**Groq Whisper API**（速度快，有免费额度）：

```bash
curl https://api.groq.com/openai/v1/audio/transcriptions \
  -H "Authorization: Bearer $GROQ_API_KEY" \
  -F file=@audio.mp3 -F model=whisper-large-v3 \
  -F response_format=verbose_json \
  -F "timestamp_granularities[]=word" \
  -o transcript-groq.json

# 将 JSON 转换为 SRT（使用转换脚本）
# 然后使用生成的 transcript.srt
```

API 返回的 JSON 格式通常包含词级时间戳，可使用 `references/transcript-to-srt.js` 脚本转换为 SRT。

### 直接请求 SRT 格式

OpenAI 和 Groq API 也支持直接返回 SRT 格式：

```bash
curl https://api.openai.com/v1/audio/transcriptions \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -F file=@audio.mp3 -F model=whisper-1 \
  -F response_format=srt \
  -o transcript.srt
```

这种方式直接输出 SRT，无需额外转换。

### 其他转录工具

任何能输出 SRT 的工具都可以配合 OpenCat 使用：

- **faster-whisper** — 更快的本地转录
- **Google Cloud Speech-to-Text** — 支持更多语言
- **Deepgram** — 实时转录 API
- **WhisperX** — 带说话人分离的转录

```bash
# 示例：使用 faster-whisper
faster-whisper audio.mp3 --model medium --output_format srt --output_dir .
```

## 如果不存在字幕文件

1. 检查项目 `references/` 目录下是否有 `.srt` 文件
2. 如果没有，运行转录 — 根据内容类型选择起始模型：
   - 语音/画外音 → `small.en`
   - 带人声的音乐 → `medium.en`
   ```bash
   whisper.cpp/main -f audio.mp3 -m whisper.cpp/models/ggml-medium.bin -osrt -of references/transcript
   ```
3. **阅读字幕并运行质量检查**（见上文）。如果失败，使用更大的模型重试或建议手动提供歌词。

## 转换脚本

如果你有带词级时间戳的 JSON 转录文件，可使用以下 Node.js 脚本转换为 SRT：

参考 `references/transcript-to-srt.js`。用法：

```bash
node references/transcript-to-srt.js transcript-openai.json transcript.srt
```
