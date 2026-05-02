# 文本转语音 (TTS)

OpenCat 不提供内置 TTS 功能。你可以使用任何外部 TTS 工具（如 Kokoro-82M、Edge TTS、ElevenLabs、OpenAI TTS 等）生成音频文件，然后在 JSONL 合成中引用。

## 推荐：使用 Kokoro-82M（免费、本地运行、无需 API Key）

如果你需要免费离线方案，推荐 [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M)。安装后使用：

```bash
# 安装依赖
pip install kokoro-onnx soundfile

# 生成语音
python -c "
from kokoro import KPipeline
from soundfile import write
pipeline = KPipeline(lang_code='a')
audio = pipeline('要合成的文本', voice='af_heart', speed=1.0)
write('narration.wav', audio, 24000)
"
```

## 声音选择

根据内容类型匹配合适的声音。以下为 Kokoro 的声音编码，其他 TTS 工具请参考其文档。

| 内容类型 | 推荐声音 | 说明 |
|---------|---------|------|
| 产品演示 | `af_heart` / `af_nova` | 温暖、专业 |
| 教程 | `am_adam` / `bf_emma` | 中性、易跟读 |
| 营销 | `af_sky` / `am_michael` | 有活力或有权威感 |
| 文档 | `bf_emma` / `bm_george` | 清晰的英式英语 |
| 日常 | `af_heart` / `af_sky` | 亲切、自然 |

Kokoro 支持 54 种声音（8 种语言），运行 `python -c "from kokoro import KPipeline; print(KPipeline.get_voices())"` 查看完整列表。

## 多语言说明

Kokoro 声音 ID 的首字母编码语言：`a`=美式英语、`b`=英式英语、`e`=西班牙语、`f`=法语、`h`=印地语、`i`=意大利语、`j`=日语、`p`=巴西葡萄牙语、`z`=中文。使用与文本语言匹配的声音即可自动选择正确的音素器。

非英语音素化需要系统安装 `espeak-ng`（macOS: `brew install espeak-ng`，Debian/Ubuntu: `apt-get install espeak-ng`）。

## 语速调节

- **0.7-0.8** — 教程、复杂内容
- **1.0** — 自然语速（默认）
- **1.1-1.2** — 开场、轻快内容
- **1.5+** — 很少适用

## 在 OpenCat 合成中使用

生成的音频文件通过 JSONL 中的 `type: "audio"` 节点引用：

```jsonl
{"id":"narration","parentId":"scene1","type":"audio","path":"narration.wav"}
```

参数说明：
- `id` — 音频节点唯一标识
- `parentId` — 所属场景 ID
- `type` — 固定为 `"audio"`
- `path` — 音频文件路径（相对于合成工作目录）

## TTS + 字幕工作流

```bash
# 第一步：使用外部 TTS 工具生成音频
python -c "
from kokoro import KPipeline
from soundfile import write
pipeline = KPipeline(lang_code='a')
audio = pipeline('你的脚本内容', voice='af_heart', speed=1.0)
write('narration.wav', audio, 24000)
"

# 第二步：使用外部工具生成字幕（如 whisper）
# whisper narration.wav --output_format srt --language zh
# 或参考 references/captions.md 获取字幕工作流
```

## 外部 TTS 工具推荐

| 工具 | 类型 | 特点 |
|------|------|------|
| Kokoro-82M | 本地、免费 | CPU 可运行，54 种声音 |
| Edge TTS | 本地、免费 | 微软语音引擎，多种语言 |
| ElevenLabs | API | 高质量、声音克隆 |
| OpenAI TTS | API | 简单易用，多语言 |
