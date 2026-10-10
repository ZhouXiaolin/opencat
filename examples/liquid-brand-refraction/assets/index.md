# Asset ledger

## Liquid pop v2 reference environment

- `equirect-garden.jpg` — exact environment image referenced by the supplied liquid-refraction demo. Source: `https://raw.githubusercontent.com/prinzipiell/tsl/main/garden-anomaly/equiRectGarden.jpg`. Usage is restricted to the Three.js scene environment/reflection pipeline; it is never drawn as a visible background.

## Official brand marks

- `heygen-prism-official.svg` — official `HeyGen_Logo_Prism_Black.svg` from the local HeyGen product brand repository. The source paths, gradients, and colors are unchanged; only the SVG viewport is cropped from the full lockup to the prism artwork.
- `hyperframes-symbol.svg` — original HyperFrames two-lobe gradient symbol from the local HyperFrames launch assets. Used unchanged.
- `heygen-wordmark-official.svg` — viewport-only crop of the official HeyGen name paths from `HeyGen_Logo_Prism_Black.svg`. Used after rupture beside the live WebGL prism; no text is retyped and no symbol is duplicated.
- `hyperframes-wordmark-official.svg` — viewport-only crop of the official HyperFrames name paths from the local full lockup. Used after rupture beside the existing symbol plane; no text is retyped and no symbol is duplicated.

## HeyGen liquid-pop sound design

- `sfx/whoosh-short.mp3` — HeyGen sound library, resolved through `media-use` and recorded in `audio_meta.json`. Source cue for the final pressure release.
- `sfx/pop.mp3` — HeyGen sound library, resolved through `media-use` and recorded in `audio_meta.json`. Source cue for the frame-84 liquid rupture.
- `sfx/sparkle.mp3` — HeyGen sound library, resolved through `media-use` and recorded in `audio_meta.json`. Source cue for the official lockup reveal.
- `audio/heygen-liquid-pop-sfx.wav` — deterministic 48kHz/24-bit stereo SFX master assembled from the three resolved cues. Whoosh runs 2.50–2.70s with a 50ms fade, pop begins at 2.80s, and sparkle runs 3.02–4.82s with a 200ms fade. Leading source silence is removed before placement; the master measures -19.2 LUFS integrated and -2.3 dBTP, preserves the 2.70–2.80s silence pocket, and is silent through the 6.0s tail.

- `heygen_prism_model` → `assets/heygen-prism.glb`
  - Provenance: user-local HeyGen prism sting source at `heygen-prism-sting/assets/model_actual.glb`.
- `heygen_prism_facet_videos` → `assets/01.mp4` through `assets/08.mp4`
  - Provenance: unchanged project-local copies of the eight facet sources from `heygen-prism-sting/assets/`.
  - Treatment: sampled live by the original prism-webgl-derived shader and mapped to the GLB face IDs; they are not flattened into an SVG or replacement texture.
- `heygen_logo_fallback` → `assets/heygen-logo.png`
  - Provenance: user-local HeyGen prism sting source at `heygen-prism-sting/assets/heygen-logo.png`.
- `hyperframes_logo` → `assets/hyperframes-logo.svg`
  - Provenance: user-local HyperFrames timeline launch video source.
- `hyperframes_symbol_alternate` → `assets/hyperframes-symbol.svg`
  - Provenance: user-local HyperFrames logo prompt video source.
- `heygen_prism_sting_video` → `assets/heygen-prism-sting.mp4`
  - Provenance: frozen render from the existing user-local HeyGen prism sting composition; 1920×1080, 9.02s, 30fps.
  - Treatment: unchanged source pixels; used as a WebGL refraction texture.
- `heygen_prism_sting_video_conformed` → `assets/heygen-prism-sting-8s.mp4`
  - Derived from the frozen sting render by conforming 9.02s to 8.0s, preserving audio pitch relationship, and adding one-second H.264 keyframes for deterministic seeking.
  - Used by the final compositions so the terminal HeyGen lockup lands upright.
- `hyperframes_logo_animation_video` → `assets/hyperframes-logo-animation.mp4`
  - Provenance: frozen render from the existing user-local HyperFrames logo animation; 1080×1080, 2.67s, 30fps.
  - Treatment: unchanged source pixels; used as a WebGL refraction texture.

All visual assets are frozen project-local copies. The three SFX files are frozen project-local downloads resolved from the HeyGen sound library; no generative media was used.
