// 双侧同源的唯一事实：ref（Edge 真 WebGPU）与 engine（core 录制 → wgpu）
// 都从这份表取 preset。组件名与 props 与 react 版完全一致（官方跨框架
// 同一 props 面），只是走 shaders/js 的声明式组件数组形态。
//
// frames: 渲染帧数；anim: 每帧推进 delta=1/30（false = 每帧 delta 0 原样
// 重画，静态案）。首帧两侧协议一致：delta 0（ref 循环 i===0 ? 0 : DELTA；
// engine 侧 initialize 落定后的首绘同为 delta 0）。
export const W = 960;
export const H = 540;
export const FPS = 30;

export const CASES = {
    solid: {
        frames: 1,
        anim: false,
        components: [
            { type: 'SolidColor', id: 'c1', props: { color: '#3366cc' } },
        ],
    },
    gradient: {
        frames: 1,
        anim: false,
        components: [
            { type: 'LinearGradient', id: 'g1', props: { colorA: '#0f172a', colorB: '#7c3aed', angle: 45 } },
        ],
    },
    noise: {
        frames: 1,
        anim: false,
        components: [
            { type: 'PerlinNoise', id: 'n1', props: { scale: 2, seed: 42 } },
        ],
    },
    blend: {
        frames: 1,
        anim: false,
        components: [
            { type: 'LinearGradient', id: 'bg', props: { colorA: '#0f172a', colorB: '#7c3aed', angle: 45 } },
            { type: 'RadialGradient', id: 'fg', props: { colorA: '#f59e0b', colorB: '#ef4444cc', blendMode: 'overlay', opacity: 0.8 } },
        ],
    },
    // 动画案：30 帧噪声演化（seed 固定，speed 交由逐帧 delta 驱动）。
    'noise-anim': {
        frames: 30,
        anim: true,
        components: [
            { type: 'PerlinNoise', id: 'n1', props: { scale: 3, seed: 42, speed: 1 } },
        ],
    },

    // ── 批次 S3：扩语料（compute 系出列——PixelSort/ReactionDiffusion/
    // Glass 等真用 compute 管线的 19 案不进 S3；ReactionDiffusion 单列
    // 为 S4 compute 验收案）──────────────────────────────────────────

    // Shapes 家族：填充 + 描边协议（softness 0 硬边 + 非零描边）。
    star: {
        frames: 1,
        anim: false,
        components: [
            { type: 'Star', id: 's1', props: { color: '#f59e0b', radius: 0.35, sides: 5, innerRatio: 0.45, rotation: 18, strokeThickness: 0.02, strokeColor: '#0f172a' } },
        ],
    },
    // Textures 家族：pattern（硬边几何 + 单元格密度）。
    checkerboard: {
        frames: 1,
        anim: false,
        components: [
            { type: 'Checkerboard', id: 'cb1', props: { colorA: '#1e293b', colorB: '#f1f5f9', cells: 8 } },
        ],
    },
    stripes: {
        frames: 1,
        anim: false,
        components: [
            { type: 'Stripes', id: 'st1', props: { colorA: '#111827', colorB: '#f9fafb', angle: 30, density: 6 } },
        ],
    },
    truchet: {
        frames: 1,
        anim: false,
        components: [
            { type: 'Truchet', id: 'tr1', props: { colorA: '#0b1220', colorB: '#38bdf8', cells: 10, seed: 3, thickness: 1.5 } },
        ],
    },
    // gradient 家族第二/三案：径向 + 角向（repeat > 1 覆盖平铺）。
    'radial-gradient': {
        frames: 1,
        anim: false,
        components: [
            { type: 'RadialGradient', id: 'rg1', props: { colorA: '#0ea5e9', colorB: '#f97316', repeat: 2 } },
        ],
    },
    'conic-gradient': {
        frames: 1,
        anim: false,
        components: [
            { type: 'ConicGradient', id: 'cg1', props: { colorA: '#ff0080', colorB: '#00bfff', repeat: 2, rotation: 45 } },
        ],
    },
    // 噪声家族扩员：Simplex / Worley（octaves 2）/ Voronoi（细胞+边）。
    'simplex-noise': {
        frames: 1,
        anim: false,
        components: [
            { type: 'SimplexNoise', id: 'sn1', props: { scale: 3, seed: 7 } },
        ],
    },
    'worley-noise': {
        frames: 1,
        anim: false,
        components: [
            { type: 'WorleyNoise', id: 'wn1', props: { scale: 6, seed: 3, octaves: 2, jitter: 0 } },
        ],
    },
    // voronoi 出列（2026-11-02）：点位 = fract(sin(大参数)×43758.5453)
    // 数位提取散列，无 jitter 关闭面——跨编译器（Tint/naga 的 mul+add
    // FMA 收缩差 → 1 ulp）经散列放大成全场点重排，内在不可对拍。
    // worley-noise 同散列但保留：jitter:0 关闭该路径（实测 0.0000%）。
    // child 协议案：filter（requiresChild）包 gradient——覆盖 childNode
    // /convertToTexture 子纹理链路（JSON components 的 children 递归面）。
    'invert-child': {
        frames: 1,
        anim: false,
        components: [
            {
                type: 'Invert', id: 'inv',
                children: [
                    { type: 'LinearGradient', id: 'lg', props: { colorA: '#0f172a', colorB: '#f59e0b', angle: 120 } },
                ],
            },
        ],
    },
    // 动画案第二式：Plasma 时基噪声演化（speed 1，30 帧 delta=1/30）。
    'plasma-anim': {
        frames: 30,
        anim: true,
        components: [
            { type: 'Plasma', id: 'pl1', props: { density: 2, speed: 1, colorA: '#7018be', colorB: '#000000' } },
        ],
    },
    // ── S2 动画时钟全案：全部 speed 消费方（PerlinNoise/SimplexNoise/
    // Plasma/Stripes；Worley/Voronoi 的 animTime 路径=散列出列）+ 长钟案
    // （90 帧累加协议验证）───────────────────────────────────────────
    'simplex-anim': {
        frames: 30,
        anim: true,
        components: [
            { type: 'SimplexNoise', id: 'sa1', props: { scale: 3, seed: 7, speed: 1 } },
        ],
    },
    'stripes-anim': {
        frames: 30,
        anim: true,
        components: [
            { type: 'Stripes', id: 'st2', props: { colorA: '#111827', colorB: '#f9fafb', angle: 30, density: 6, speed: 1 } },
        ],
    },
    // 长钟：90 帧 = 3 秒钟累加，验证推钟协议无漂移/无跳步。
    'noise-anim-long': {
        frames: 90,
        anim: true,
        components: [
            { type: 'PerlinNoise', id: 'nl1', props: { scale: 3, seed: 42, speed: 1 } },
        ],
    },
    // worley-anim 出列（2026-11-02）：细胞漂移动画本身走 animTime+sin
    // 抖动路径（同 voronoi 散列），关闭动画=静态图，该案无对拍面。
    // ── S4 compute 验收案：Gray-Scott 反应扩散（TypeGPU compute 管线，
    // 无 child 无交互）。初始态由核过程生成（无 Math.random），种子核
    // 纯 IEEE 单步运算，跨编译器逐位一致。
    //
    // 单帧 + speed 32 的两个约束来源：
    // 1) 双编译器舍入差：迭代核 mul+add 链在 Dawn(Tint) 与 wgpu(naga)
    //    的 FMA 收缩决策不同 → 每次迭代引入末位差，反应扩散生长期对
    //    末位差指数放大（speed 6 × 30 帧实测 avg_p8 19.8%，分歧窗口
    //    集中在成斑期）——帧数必须压到 1；
    // 2) mp4 编码污染：更低速的态 = 稀疏 1px 点 = 4:2:0 色度最劣形态
    //    （speed 2 单帧 mp4 14.6% vs PNG 直对 0.0000%/maxd2），speed 32
    //    让点长成团斑，编码可行。32 次 dispatch 同时覆盖 ping-pong
    //    两个方向的 bind group 交替。
    'reaction-diffusion': {
        frames: 1,
        anim: false,
        components: [
            { type: 'ReactionDiffusion', id: 'rd1', props: { preset: 'coral', speed: 32 } },
        ],
    },    // ── 批次 S6：纯 fragment 源效果 30 案（props=registry default；散列出列见 gen-presets-batch.mjs）──
    "Arc": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Arc",
                "id": "s6-arc",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "radius": 0.38,
                        "aperture": 270,
                        "rotation": 0,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "Beam": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Beam",
                "id": "s6-beam",
                "props": {
                        "startPosition": {
                                "x": 0.2,
                                "y": 0.5
                        },
                        "endPosition": {
                                "x": 0.8,
                                "y": 0.5
                        },
                        "startThickness": 0.2,
                        "endThickness": 0.2,
                        "startSoftness": 0.5,
                        "endSoftness": 0.5,
                        "insideColor": "#FF0000",
                        "outsideColor": "#0000FF",
                        "colorSpace": "linear"
                }
        }],
    },
    "BlueNoise": {
        frames: 1,
        anim: false,
        components: [{
                "type": "BlueNoise",
                "id": "s6-bluenoise",
                "props": {
                        "colorA": "#ffffff",
                        "colorB": "#000000",
                        "stops": null,
                        "colorSpace": "linear",
                        "grain": 1,
                        "contrast": 0,
                        "balance": 0,
                        "seed": 0
                }
        }],
    },
    "CarbonFiber": {
        frames: 1,
        anim: false,
        components: [{
                "type": "CarbonFiber",
                "id": "s6-carbonfiber",
                "props": {
                        "origin": "center",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "scale": 1,
                        "rotation": 0,
                        "lightColor": "#b9bdc6",
                        "darkColor": "#0b0c0e",
                        "weaveStyle": "twill",
                        "weaveScale": 30,
                        "weaveAngle": 0,
                        "relief": 0.5,
                        "fiberSheen": 1.2,
                        "roughness": 0.5,
                        "clearcoat": 1.2,
                        "environment": 2,
                        "envRotation": 0,
                        "lightAngle": 215,
                        "bevelWidth": 0.05,
                        "bevelShape": 0,
                        "edgeSoftness": 0.05,
                        "shape": "{\"type\":\"sphere3D\",\"radius\":0.35}",
                        "shapeSdfUrl": "",
                        "shapeType": ""
                }
        }],
    },
    "Circle": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Circle",
                "id": "s6-circle",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "radius": 1,
                        "softness": 0,
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "Crescent": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Crescent",
                "id": "s6-crescent",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "radius": 0.3,
                        "innerRatio": 0.8,
                        "offset": 0.2,
                        "rotation": 0,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "Cross": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Cross",
                "id": "s6-cross",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "radius": 0.35,
                        "thickness": 0.08,
                        "rounding": 0,
                        "rotation": 0,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "DiamondGradient": {
        frames: 1,
        anim: false,
        components: [{
                "type": "DiamondGradient",
                "id": "s6-diamondgradient",
                "props": {
                        "colorA": "#4ffb4a",
                        "colorB": "#4f1238",
                        "stops": null,
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "size": 0.7,
                        "rotation": 0,
                        "repeat": 1,
                        "roundness": 0,
                        "colorSpace": "linear"
                }
        }],
    },
    "Ellipse": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Ellipse",
                "id": "s6-ellipse",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "radiusX": 0.35,
                        "radiusY": 0.2,
                        "rotation": 0,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "ErosionNoise": {
        frames: 1,
        anim: false,
        components: [{
                "type": "ErosionNoise",
                "id": "s6-erosionnoise",
                "props": {
                        "colorA": "#ffffff",
                        "colorB": "#000000",
                        "stops": null,
                        "colorSpace": "linear",
                        "scale": 1.5,
                        "contrast": 0,
                        "balance": 0,
                        "seed": 0
                }
        }],
    },
    "Flower": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Flower",
                "id": "s6-flower",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "radius": 0.4,
                        "sides": 5,
                        "innerRatio": 0.4,
                        "rotation": 0,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "Grid": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Grid",
                "id": "s6-grid",
                "props": {
                        "color": "#ffffff",
                        "cellColor": "transparent",
                        "cells": 10,
                        "thickness": 1,
                        "rotation": 0,
                        "softness": 0,
                        "variation": 0,
                        "colorSpace": "linear"
                }
        }],
    },
    "Heart": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Heart",
                "id": "s6-heart",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "radius": 0.32,
                        "rotation": 0,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "HexGrid": {
        frames: 1,
        anim: false,
        components: [{
                "type": "HexGrid",
                "id": "s6-hexgrid",
                "props": {
                        "colorA": "#000000",
                        "colorB": "#ffffff",
                        "cells": 8,
                        "thickness": 1,
                        "rotation": 0,
                        "softness": 0,
                        "variation": 0,
                        "colorSpace": "linear"
                }
        }],
    },
    "Irradiance": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Irradiance",
                "id": "s6-irradiance",
                "props": {
                        "origin": "center",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "scale": 1,
                        "rotation": 0,
                        "lights": [
                                {
                                        "position": {
                                                "x": 0.3,
                                                "y": 0.3
                                        },
                                        "color": "#ffb347",
                                        "intensity": 3
                                },
                                {
                                        "position": {
                                                "x": 0.72,
                                                "y": 0.7
                                        },
                                        "color": "#7a5cff",
                                        "intensity": 2
                                }
                        ],
                        "lightHeight": 0.5,
                        "lightRange": 1.2,
                        "reach": 3,
                        "core": 1,
                        "wrap": 0.1,
                        "shadows": true,
                        "shadowSoftness": 0.15,
                        "bodyColor": "#000000",
                        "bodyLight": 0.6,
                        "bevelWidth": 0.01,
                        "edgeSoftness": 0.05,
                        "shape": "{\"type\":\"sphere3D\",\"radius\":0.35}",
                        "shapeSdfUrl": "",
                        "shapeType": ""
                }
        }],
    },
    // IsometricCubes 出列（2026-11-02）：线条 AA 半像素差 × 全屏密集线网（cells=6）
    // 面积放大（web 2.29%）；thickness 1→2 实测 2.58% 反向——无参数关闭面。
    "IsometricCubes": {
        frames: 1,
        anim: false,
        components: [{
                "type": "IsometricCubes",
                "id": "s6-isometriccubes",
                "props": {
                        "colorA": "#7c5cff",
                        "colorB": "#ff5c9d",
                        "lineColor": "#000000",
                        "cells": 6,
                        "thickness": 1,
                        "rotation": 0,
                        "softness": 0,
                        "colorVariation": 1,
                        "colorSpace": "linear"
                }
        }],
    },
    "Line": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Line",
                "id": "s6-line",
                "props": {
                        "color": "#ffffff",
                        "pointA": {
                                "x": 0.2,
                                "y": 0.5
                        },
                        "pointB": {
                                "x": 0.8,
                                "y": 0.5
                        },
                        "thickness": 0.01,
                        "style": "solid",
                        "dashLength": 0.05,
                        "gapLength": 0.025,
                        "capStart": "rounded",
                        "capEnd": "rounded"
                }
        }],
    },
    "MultiPointGradient": {
        frames: 1,
        anim: false,
        components: [{
                "type": "MultiPointGradient",
                "id": "s6-multipointgradient",
                "props": {
                        "colorA": "#4776E6",
                        "positionA": {
                                "x": 0.2,
                                "y": 0.2
                        },
                        "colorB": "#C44DFF",
                        "positionB": {
                                "x": 0.8,
                                "y": 0.2
                        },
                        "colorC": "#1ABC9C",
                        "positionC": {
                                "x": 0.2,
                                "y": 0.8
                        },
                        "colorD": "#F8BBD9",
                        "positionD": {
                                "x": 0.8,
                                "y": 0.8
                        },
                        "colorE": "#FF8C42",
                        "positionE": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "colorSpace": "linear",
                        "smoothness": 2
                }
        }],
    },
    "Neon": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Neon",
                "id": "s6-neon",
                "props": {
                        "origin": "center",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "scale": 1,
                        "rotation": 0,
                        "color": "#00ddff",
                        "secondaryColor": "#ff00aa",
                        "secondaryBlend": 0.5,
                        "glowColor": "#00ddff",
                        "tubeThickness": 0.2,
                        "intensity": 1.5,
                        "hotCoreIntensity": 0.6,
                        "glowIntensity": 0.6,
                        "glowRadius": 0.25,
                        "lightAngle": 300,
                        "specularIntensity": 0.5,
                        "specularSize": 0.5,
                        "cornerSmoothing": 0.15,
                        "flickerSpeed": 0,
                        "flickerAmount": 0.2,
                        "flowSpeed": 0,
                        "flowAmount": 0.3,
                        "shape": "{\"type\":\"sphere3D\",\"radius\":0.35}",
                        "shapeSdfUrl": "",
                        "shapeType": ""
                }
        }],
    },
    "Parallelogram": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Parallelogram",
                "id": "s6-parallelogram",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "width": 0.32,
                        "height": 0.22,
                        "skew": 0.15,
                        "rotation": 0,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    // Particles 出列（2026-11-02）：TypeGPU compute 力场（force.turbulence）——
    // FMA 收缩分歧内在极限①，engine 12.36%。
    "Particles": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Particles",
                "id": "s6-particles",
                "props": {
                        "origin": "center",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "scale": 1,
                        "rotation": 0,
                        "colorA": "#8ec5ff",
                        "colorB": "#ff7ad9",
                        "count": 4000,
                        "size": 1.5,
                        "particleShape": "dot",
                        "spread": 1,
                        "agitation": 0.12,
                        "damping": 0.4,
                        "gravity": 0,
                        "mouseInfluence": 2,
                        "mouseRadius": 0.2,
                        "exposure": 1,
                        "softness": 0.1,
                        "depth": 0.18,
                        "colorSpace": "oklab",
                        "shape": "{\"type\":\"sphere3D\",\"radius\":0.35}",
                        "shapeSdfUrl": "",
                        "shapeType": ""
                }
        }],
    },
    "Polygon": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Polygon",
                "id": "s6-polygon",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "radius": 0.4,
                        "sides": 6,
                        "rounding": 0,
                        "rotation": 0,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "Ring": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Ring",
                "id": "s6-ring",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "radius": 0.3,
                        "thickness": 0.07,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "RoundedRect": {
        frames: 1,
        anim: false,
        components: [{
                "type": "RoundedRect",
                "id": "s6-roundedrect",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "width": 0.35,
                        "height": 0.25,
                        "rounding": 0.05,
                        "rotation": 0,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "StudioBackground": {
        frames: 1,
        anim: false,
        components: [{
                "type": "StudioBackground",
                "id": "s6-studiobackground",
                "props": {
                        "color": "#d8dbec",
                        "keyColor": "#d5e4ea",
                        "keyIntensity": 40,
                        "keySoftness": 50,
                        "fillColor": "#d5e4ea",
                        "fillIntensity": 10,
                        "fillSoftness": 70,
                        "fillAngle": 70,
                        "backColor": "#c8d4e8",
                        "backIntensity": 20,
                        "backSoftness": 80,
                        "brightness": 20,
                        "vignette": 0,
                        "center": {
                                "x": 0.5,
                                "y": 0.8
                        },
                        "lightTarget": 100,
                        "wallCurvature": 10,
                        "ambientIntensity": 50,
                        "ambientSpeed": 2,
                        "seed": 0
                }
        }],
    },
    "Teardrop": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Teardrop",
                "id": "s6-teardrop",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "radius": 0.22,
                        "height": 0.4,
                        "rotation": 0,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "Trapezoid": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Trapezoid",
                "id": "s6-trapezoid",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "bottomWidth": 0.35,
                        "topWidth": 0.2,
                        "height": 0.25,
                        "rotation": 0,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "Vesica": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Vesica",
                "id": "s6-vesica",
                "props": {
                        "origin": "center",
                        "color": "#ffffff",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "radius": 0.35,
                        "spread": 0.5,
                        "rotation": 0,
                        "softness": 0,
                        "strokeThickness": 0,
                        "strokeColor": "#000000",
                        "strokePosition": "center",
                        "colorSpace": "linear"
                }
        }],
    },
    "Voxels": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Voxels",
                "id": "s6-voxels",
                "props": {
                        "origin": "center",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "scale": 1,
                        "rotation": 0,
                        "voxelSize": 0.035,
                        "voxelShape": "cube",
                        "voxelScale": 1,
                        "fill": 0,
                        "bevel": 0.08,
                        "depth": 0.12,
                        "gridSpace": "shape",
                        "colorA": "#6ea8ff",
                        "colorB": "#1d2b4f",
                        "colorMode": "height",
                        "colorVariation": 0.25,
                        "colorSpace": "oklab",
                        "lightAngle": 225,
                        "lightElevation": 42,
                        "lightColor": "#fff2df",
                        "lightIntensity": 1.1,
                        "ambientColor": "#9fb4d8",
                        "ambient": 0.55,
                        "shadows": 0.85,
                        "shadowSoftness": 0.35,
                        "ao": 1,
                        "glossiness": 0.35,
                        "specular": 0.6,
                        "seams": 0.2,
                        "shape": "{\"type\":\"sphere3D\",\"radius\":0.35}",
                        "shapeSdfUrl": "",
                        "shapeType": ""
                }
        }],
    },
    "Weave": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Weave",
                "id": "s6-weave",
                "props": {
                        "colorA": "#c4c4c4",
                        "colorB": "#4d4d4d",
                        "cells": 10,
                        "gap": 0.25,
                        "rotation": 0
                }
        }],
    },
    // ── 批次 S8：filter 批量 76 案（child=LinearGradient 基线底图）（props=registry default；散列出列见 gen-presets-batch.mjs）──
    "AngularBlur": {
        frames: 1,
        anim: false,
        components: [{
                "type": "AngularBlur",
                "id": "s8-angularblur",
                "props": {
                        "intensity": 20,
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "dispersion": 0,
                        "falloff": 0,
                        "focus": 0,
                        "bias": 0,
                        "highlights": 0,
                        "jitter": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-angularblur-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    // Ascii 出列（2026-11-02）：engine 侧 glyph atlas 缺口全白（引擎工程债，非协议面）；
    // web 腿渲染 exit 2 同因。
    "Ascii": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Ascii",
                "id": "s8-ascii",
                "props": {
                        "characters": "@%#*+=-:.",
                        "cellSize": 30,
                        "fontFamily": "JetBrains Mono",
                        "spacing": 1,
                        "gamma": 1,
                        "alphaThreshold": 0,
                        "preserveAlpha": true
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-ascii-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "BarShift": {
        frames: 1,
        anim: false,
        components: [{
                "type": "BarShift",
                "id": "s8-barshift",
                "props": {
                        "count": 6,
                        "angle": 0,
                        "intensity": 0.15,
                        "seed": 0,
                        "speed": 0,
                        "edges": "mirror"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-barshift-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Bend": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Bend",
                "id": "s8-bend",
                "props": {
                        "strength": 0.5,
                        "falloff": 0,
                        "angle": 0,
                        "edges": "transparent"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-bend-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Blur": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Blur",
                "id": "s8-blur",
                "props": {
                        "intensity": 50
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-blur-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "BokehBlur": {
        frames: 1,
        anim: false,
        components: [{
                "type": "BokehBlur",
                "id": "s8-bokehblur",
                "props": {
                        "radius": 50,
                        "highlightGain": 4,
                        "highlightThreshold": 0.6,
                        "bladeShape": "blades",
                        "bladeCount": 6,
                        "bladeRotation": 0,
                        "chromaticFringe": 0.2,
                        "catEye": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-bokehblur-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "BrightnessContrast": {
        frames: 1,
        anim: false,
        components: [{
                "type": "BrightnessContrast",
                "id": "s8-brightnesscontrast",
                "props": {
                        "brightness": 0,
                        "contrast": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-brightnesscontrast-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Bulge": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Bulge",
                "id": "s8-bulge",
                "props": {
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "strength": 1,
                        "radius": 1,
                        "falloff": 0.5,
                        "edges": "stretch"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-bulge-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Chalkboard": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Chalkboard",
                "id": "s8-chalkboard",
                "props": {
                        "boardColor": "#000000",
                        "chalkColor": "#eceadb",
                        "edgeSensitivity": 0.5,
                        "edgeThickness": 1.5,
                        "shading": 0.05,
                        "hatchScale": 16,
                        "grain": 0.45
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-chalkboard-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "ChannelBlur": {
        frames: 1,
        anim: false,
        components: [{
                "type": "ChannelBlur",
                "id": "s8-channelblur",
                "props": {
                        "redIntensity": 0,
                        "greenIntensity": 20,
                        "blueIntensity": 40
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-channelblur-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "ChromaticAberration": {
        frames: 1,
        anim: false,
        components: [{
                "type": "ChromaticAberration",
                "id": "s8-chromaticaberration",
                "props": {
                        "strength": 0.2,
                        "angle": 0,
                        "redOffset": -1,
                        "greenOffset": 0,
                        "blueOffset": 1
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-chromaticaberration-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "CompressionArtifacts": {
        frames: 1,
        anim: false,
        components: [{
                "type": "CompressionArtifacts",
                "id": "s8-compressionartifacts",
                "props": {
                        "quality": 12
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-compressionartifacts-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "ConcentricSpin": {
        frames: 1,
        anim: false,
        components: [{
                "type": "ConcentricSpin",
                "id": "s8-concentricspin",
                "props": {
                        "intensity": 20,
                        "rings": 8,
                        "smoothness": 0.03,
                        "seed": 0,
                        "speed": 0.1,
                        "speedRandomness": 0.5,
                        "edges": "mirror",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        }
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-concentricspin-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "ContourLines": {
        frames: 1,
        anim: false,
        components: [{
                "type": "ContourLines",
                "id": "s8-contourlines",
                "props": {
                        "levels": 5,
                        "lineWidth": 2,
                        "softness": 0,
                        "gamma": 0.5,
                        "invert": false,
                        "source": "luminance",
                        "colorMode": "source",
                        "lineColor": "#000000",
                        "backgroundColor": "transparent"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-contourlines-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "CornerPin": {
        frames: 1,
        anim: false,
        components: [{
                "type": "CornerPin",
                "id": "s8-cornerpin",
                "props": {
                        "topLeft": {
                                "x": 0,
                                "y": 0
                        },
                        "topRight": {
                                "x": 1,
                                "y": 0
                        },
                        "bottomLeft": {
                                "x": 0,
                                "y": 1
                        },
                        "bottomRight": {
                                "x": 1,
                                "y": 1
                        },
                        "amount": 1,
                        "edges": "transparent"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-cornerpin-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "CRTScreen": {
        frames: 1,
        anim: false,
        components: [{
                "type": "CRTScreen",
                "id": "s8-crtscreen",
                "props": {
                        "pixelSize": 128,
                        "colorShift": 1,
                        "scanlineIntensity": 0.3,
                        "scanlineFrequency": 200,
                        "brightness": 1,
                        "contrast": 1,
                        "vignetteIntensity": 1,
                        "vignetteRadius": 0.5
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-crtscreen-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "DataMosh": {
        frames: 1,
        anim: false,
        components: [{
                "type": "DataMosh",
                "id": "s8-datamosh",
                "props": {
                        "intensity": 0.7,
                        "blockSize": 48,
                        "drift": 0.35,
                        "churn": 0.4,
                        "blend": 1,
                        "speed": 1,
                        "seed": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-datamosh-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "DiffuseBlur": {
        frames: 1,
        anim: false,
        components: [{
                "type": "DiffuseBlur",
                "id": "s8-diffuseblur",
                "props": {
                        "intensity": 30,
                        "edges": "stretch",
                        "chromatic": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-diffuseblur-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "DisplacementMap": {
        frames: 1,
        anim: false,
        components: [{
                "type": "DisplacementMap",
                "id": "s8-displacementmap",
                "props": {
                        "source": "",
                        "amount": 0.3,
                        "channelMode": "twoAxis",
                        "angle": 0,
                        "edges": "mirror"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-displacementmap-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Dither": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Dither",
                "id": "s8-dither",
                "props": {
                        "pattern": "bayer4",
                        "pixelSize": 4,
                        "threshold": 0.5,
                        "spread": 1,
                        "colorMode": "custom",
                        "colorA": "transparent",
                        "colorB": "#ffffff"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-dither-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "DropShadow": {
        frames: 1,
        anim: false,
        components: [{
                "type": "DropShadow",
                "id": "s8-dropshadow",
                "props": {
                        "color": "#000000",
                        "distance": 0.1,
                        "angle": 135,
                        "blur": 5,
                        "intensity": 0.5,
                        "cutout": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-dropshadow-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Duotone": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Duotone",
                "id": "s8-duotone",
                "props": {
                        "colorA": "#ff0000",
                        "colorB": "#023af4",
                        "blend": 0.5,
                        "colorSpace": "linear"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-duotone-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Emboss": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Emboss",
                "id": "s8-emboss",
                "props": {
                        "origin": "center",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "scale": 1,
                        "rotation": 0,
                        "depth": -0.5,
                        "lightAngle": 260,
                        "lightIntensity": 0.6,
                        "shadowIntensity": 0.3,
                        "shape": "{\"type\":\"sphere3D\",\"radius\":0.35}",
                        "shapeSdfUrl": "",
                        "shapeType": ""
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-emboss-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Engraving": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Engraving",
                "id": "s8-engraving",
                "props": {
                        "style": "crosshatch",
                        "frequency": 90,
                        "angle": 8,
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "relief": 0.6,
                        "waviness": 0.35,
                        "contrast": 1.15,
                        "inkColor": "#1a1410",
                        "paperColor": "#f4eee2"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-engraving-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Exposure": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Exposure",
                "id": "s8-exposure",
                "props": {
                        "exposure": 1
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-exposure-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    // FilmGrain 出列（2026-11-02）：对拍口径债非渲染分歧——web 腿 mp4 走页内 WebCodecs
    // 编码 vs ref ffmpeg crf0，编码器不同源 × 全屏高频噪声（G<R<B 通道差=4:2:0 色度
    // 特征；max diff 20 均值零偏；engine 腿同渲染 ffmpeg 链 0.0000% 背书渲染无分歧）。
    "FilmGrain": {
        frames: 1,
        anim: false,
        components: [{
                "type": "FilmGrain",
                "id": "s8-filmgrain",
                "props": {
                        "strength": 0.5,
                        "bias": 2,
                        "animated": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-filmgrain-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "FilmStock": {
        frames: 1,
        anim: false,
        components: [{
                "type": "FilmStock",
                "id": "s8-filmstock",
                "props": {
                        "stock": "portrait400",
                        "strength": 1,
                        "halation": 0.4,
                        "halationRadius": 28,
                        "weave": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-filmstock-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Flip": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Flip",
                "id": "s8-flip",
                "props": {
                        "flipX": false,
                        "flipY": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-flip-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "FlowField": {
        frames: 1,
        anim: false,
        components: [{
                "type": "FlowField",
                "id": "s8-flowfield",
                "props": {
                        "strength": 0.15,
                        "detail": 2,
                        "speed": 0,
                        "evolutionSpeed": 0,
                        "seed": 0,
                        "edges": "mirror"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-flowfield-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "FlutedGlass": {
        frames: 1,
        anim: false,
        components: [{
                "type": "FlutedGlass",
                "id": "s8-flutedglass",
                "props": {
                        "shape": "bars",
                        "angle": 0,
                        "frequency": 10,
                        "softness": 0.5,
                        "waveAmplitude": 0.06,
                        "waveFrequency": 1.5,
                        "speed": 0,
                        "refraction": 1.5,
                        "aberration": 0.2,
                        "lightAngle": 30,
                        "highlight": 0.2,
                        "highlightSoftness": 0.3,
                        "highlightColor": "#ffffff",
                        "edges": "mirror"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-flutedglass-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Form3D": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Form3D",
                "id": "s8-form3d",
                "props": {
                        "shape3d": "{\"type\":\"ribbon\",\"angle\":0,\"twist\":50,\"width\":40,\"thickness\":20,\"seed\":0}",
                        "shape3dType": "ribbon",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "zoom": 50,
                        "glossiness": 50,
                        "lighting": 50,
                        "uvMode": "stretch",
                        "speed": 1
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-form3d-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Glass": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Glass",
                "id": "s8-glass",
                "props": {
                        "origin": "center",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "scale": 1,
                        "rotation": 0,
                        "cutout": false,
                        "refraction": 1,
                        "edgeSoftness": 0.1,
                        "blur": 0,
                        "thickness": 0.2,
                        "aberration": 0.5,
                        "innerZoom": 1,
                        "lightAngle": 300,
                        "highlight": 0.05,
                        "highlightColor": "#ffffff",
                        "highlightSoftness": 0.5,
                        "fresnel": 0.1,
                        "fresnelSoftness": 0.1,
                        "fresnelColor": "#ffffff",
                        "tintColor": "#ffffff",
                        "tintIntensity": 0,
                        "tintPreserveLuminosity": true,
                        "shape": "{\"type\":\"sphere3D\",\"radius\":0.35}",
                        "shapeSdfUrl": "",
                        "shapeType": ""
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-glass-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "GlassTiles": {
        frames: 1,
        anim: false,
        components: [{
                "type": "GlassTiles",
                "id": "s8-glasstiles",
                "props": {
                        "intensity": 2,
                        "tileCount": 20,
                        "rotation": 0,
                        "roundness": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-glasstiles-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Glitch": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Glitch",
                "id": "s8-glitch",
                "props": {
                        "intensity": 0.5,
                        "speed": 1,
                        "rgbShift": 5,
                        "blockDensity": 10,
                        "colorBarIntensity": 0.2,
                        "mirrorAmount": 0.3,
                        "scanlineIntensity": 0.2
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-glitch-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Glow": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Glow",
                "id": "s8-glow",
                "props": {
                        "intensity": 1,
                        "threshold": 0.5,
                        "size": 25
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-glow-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "GradientMap": {
        frames: 1,
        anim: false,
        components: [{
                "type": "GradientMap",
                "id": "s8-gradientmap",
                "props": {
                        "palette": "rainbow",
                        "colorLow": "#1a0b2e",
                        "colorMid": "#e94560",
                        "colorHigh": "#f9ed69",
                        "speed": 0.15,
                        "contrast": 1,
                        "blackPoint": 0,
                        "whitePoint": 1,
                        "strength": 1,
                        "colorSpace": "oklch"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-gradientmap-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Grayscale": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Grayscale",
                "id": "s8-grayscale",
                "props": {},
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-grayscale-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    // Halftone 出列（2026-11-02）：亮度→点直径阈值映射对 child 亮度微差是放大器
    // （eng 点径系统性大 2 倍；frequency 40 实测 48.37% 更差——粗点阵放大更刺眼）。
    // mp4 口径 2.51% / PNG 口径 22.2% 均超阈，跨引擎不可关断。
    "Halftone": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Halftone",
                "id": "s8-halftone",
                "props": {
                        "style": "classic",
                        "frequency": 100,
                        "angle": 45,
                        "cyanAngle": 15,
                        "magentaAngle": 75,
                        "yellowAngle": 0,
                        "blackAngle": 45,
                        "misprint": 0,
                        "misprintAngle": 0,
                        "paperColor": "#ffffff",
                        "cyanColor": "#00ffff",
                        "magentaColor": "#ff00ff",
                        "yellowColor": "#ffff00",
                        "blackColor": "#000000"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-halftone-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "HueShift": {
        frames: 1,
        anim: false,
        components: [{
                "type": "HueShift",
                "id": "s8-hueshift",
                "props": {
                        "shift": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-hueshift-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Kaleidoscope": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Kaleidoscope",
                "id": "s8-kaleidoscope",
                "props": {
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "segments": 6,
                        "angle": 0,
                        "edges": "mirror"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-kaleidoscope-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "KeyFrames": {
        frames: 1,
        anim: false,
        components: [{
                "type": "KeyFrames",
                "id": "s8-keyframes",
                "props": {
                        "trackers": 12,
                        "detect": "bright",
                        "threshold": 0.15,
                        "variance": 0.8,
                        "agility": 0.5,
                        "lifespan": 6,
                        "trail": 1,
                        "markerSize": 28,
                        "lineWidth": 1.5,
                        "markerColor": "#54d66b",
                        "keyframeColor": "#ffcf33",
                        "pathColor": "#ffffffb8"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-keyframes-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "LensDistortion": {
        frames: 1,
        anim: false,
        components: [{
                "type": "LensDistortion",
                "id": "s8-lensdistortion",
                "props": {
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "spread": 0.6,
                        "angle": 0,
                        "perspective": 0.1,
                        "bias": 1,
                        "count": 35,
                        "dispersion": 1,
                        "dispersionShift": 0,
                        "dispersionColor": 0.6,
                        "focusCenter": 0.8,
                        "focusEdges": 1,
                        "lensBulge": 0,
                        "lensCircle": 0,
                        "swirl": 0.35,
                        "noise": 0,
                        "noiseFrequency": 0.25,
                        "noiseOffset": 0,
                        "grainMixer": 0,
                        "grainOverlay": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-lensdistortion-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "LightLeak": {
        frames: 1,
        anim: false,
        components: [{
                "type": "LightLeak",
                "id": "s8-lightleak",
                "props": {
                        "position": {
                                "x": 0.95,
                                "y": 0.4
                        },
                        "spread": 0.55,
                        "intensity": 0.15,
                        "streaks": 0.6,
                        "colorHot": "#fff3c4",
                        "colorMid": "#ff7a2f",
                        "colorFringe": "#a63d8f",
                        "flicker": 0.35,
                        "speed": 1,
                        "seed": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-lightleak-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "LinearBlur": {
        frames: 1,
        anim: false,
        components: [{
                "type": "LinearBlur",
                "id": "s8-linearblur",
                "props": {
                        "intensity": 30,
                        "angle": 0,
                        "dispersion": 0,
                        "falloff": 0,
                        "focus": 0,
                        "bias": 0,
                        "highlights": 0,
                        "jitter": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-linearblur-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Mirror": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Mirror",
                "id": "s8-mirror",
                "props": {
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "angle": 0,
                        "edges": "mirror"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-mirror-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "ObjectTracker": {
        frames: 1,
        anim: false,
        components: [{
                "type": "ObjectTracker",
                "id": "s8-objecttracker",
                "props": {
                        "detectionMode": "bright",
                        "threshold": 0.25,
                        "layout": "mosaic",
                        "cellSize": 400,
                        "maxDepth": 2,
                        "boxStyle": "corners",
                        "lineWidth": 1.5,
                        "cornerRadius": 0,
                        "strokeColor": "#ffffff",
                        "fillColor": "#5a79911a",
                        "labelColor": "#000000",
                        "labelBackgroundColor": "#ffffff",
                        "labelMode": "none",
                        "labelPosition": "bottom-right",
                        "labelRadius": 0,
                        "labelInset": true,
                        "fontFamily": "Inter",
                        "fontWeight": 500,
                        "fontSize": 0.015,
                        "letterSpacing": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-objecttracker-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Paper": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Paper",
                "id": "s8-paper",
                "props": {
                        "roughness": 0.3,
                        "grainScale": 1,
                        "displacement": 0.15,
                        "seed": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-paper-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "ParticleField": {
        frames: 1,
        anim: false,
        components: [{
                "type": "ParticleField",
                "id": "s8-particlefield",
                "props": {
                        "count": 15000,
                        "depthSource": "luminance",
                        "depth": 0.7,
                        "particleShape": "dot",
                        "particleSize": 0.75,
                        "depthShading": 0.6,
                        "wobble": 0.35,
                        "zoom": 1,
                        "rotationX": 0,
                        "rotationY": 0,
                        "rotationZ": 0,
                        "offsetX": 0,
                        "offsetY": 0,
                        "cursorMode": "push",
                        "cursorStrength": 0.8,
                        "cursorRadius": 0.2
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-particlefield-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Perspective": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Perspective",
                "id": "s8-perspective",
                "props": {
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "pan": 0,
                        "tilt": 0,
                        "fov": 60,
                        "zoom": 1,
                        "offset": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "edges": "transparent"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-perspective-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Pixelate": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Pixelate",
                "id": "s8-pixelate",
                "props": {
                        "scale": 50,
                        "gap": 0,
                        "roundness": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-pixelate-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "PolarCoordinates": {
        frames: 1,
        anim: false,
        components: [{
                "type": "PolarCoordinates",
                "id": "s8-polarcoordinates",
                "props": {
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "wrap": 1,
                        "radius": 1,
                        "intensity": 1,
                        "edges": "transparent"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-polarcoordinates-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Posterize": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Posterize",
                "id": "s8-posterize",
                "props": {
                        "intensity": 5
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-posterize-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "ProgressiveBlur": {
        frames: 1,
        anim: false,
        components: [{
                "type": "ProgressiveBlur",
                "id": "s8-progressiveblur",
                "props": {
                        "intensity": 50,
                        "angle": 0,
                        "center": {
                                "x": 0,
                                "y": 0.5
                        },
                        "falloff": 1,
                        "dispersion": 0,
                        "jitter": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-progressiveblur-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "RectangularCoordinates": {
        frames: 1,
        anim: false,
        components: [{
                "type": "RectangularCoordinates",
                "id": "s8-rectangularcoordinates",
                "props": {
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "scale": 1,
                        "intensity": 1,
                        "edges": "transparent"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-rectangularcoordinates-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "ReflectivePlane": {
        frames: 1,
        anim: false,
        components: [{
                "type": "ReflectivePlane",
                "id": "s8-reflectiveplane",
                "props": {
                        "height": 0.7,
                        "distance": 0.5,
                        "falloff": 0.5,
                        "blur": 3,
                        "blurDistance": 0.3,
                        "edges": "stretch"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-reflectiveplane-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Repeater": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Repeater",
                "id": "s8-repeater",
                "props": {
                        "mode": "grid",
                        "cropLeft": 0,
                        "cropRight": 0,
                        "cropTop": 0,
                        "cropBottom": 0,
                        "columns": 3,
                        "rows": 3,
                        "gapX": 0.05,
                        "gapY": 0.05,
                        "stagger": 0,
                        "flip": "none",
                        "count": 8,
                        "radius": 0.3,
                        "startAngle": 0,
                        "sweep": 360,
                        "faceCenter": false,
                        "direction": 0,
                        "spacing": 0.2,
                        "instanceScale": 1,
                        "instanceRotation": 0,
                        "instanceOpacity": 1,
                        "hueShift": 0,
                        "phase": 0,
                        "zOrder": "forward",
                        "jitterPosition": 0,
                        "jitterRotation": 0,
                        "jitterScale": 0,
                        "jitterOpacity": 0,
                        "seed": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-repeater-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Saturation": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Saturation",
                "id": "s8-saturation",
                "props": {
                        "intensity": 1
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-saturation-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Sharpness": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Sharpness",
                "id": "s8-sharpness",
                "props": {
                        "sharpness": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-sharpness-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Solarize": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Solarize",
                "id": "s8-solarize",
                "props": {
                        "threshold": 0.5,
                        "strength": 1
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-solarize-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Sparkle": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Sparkle",
                "id": "s8-sparkle",
                "props": {
                        "size": 100,
                        "intensity": 10,
                        "threshold": 0.2,
                        "expand": 0,
                        "rayLength": 5,
                        "colorize": 0.45,
                        "speed": 1,
                        "seed": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-sparkle-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Spherize": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Spherize",
                "id": "s8-spherize",
                "props": {
                        "radius": 1,
                        "depth": 1,
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "lightPosition": {
                                "x": 0.3,
                                "y": 0.3
                        },
                        "lightIntensity": 0.5,
                        "lightSoftness": 0.5,
                        "lightColor": "#ffffff"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-spherize-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Stone": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Stone",
                "id": "s8-stone",
                "props": {
                        "intensity": 0.5,
                        "scale": 1,
                        "contrast": 0,
                        "distortion": 0.15,
                        "seed": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-stone-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Stretch": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Stretch",
                "id": "s8-stretch",
                "props": {
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "strength": 1,
                        "angle": 0,
                        "falloff": 0,
                        "edges": "stretch"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-stretch-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Surface3D": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Surface3D",
                "id": "s8-surface3d",
                "props": {
                        "amplitude": 0.3,
                        "waveType": "fractal",
                        "frequency": 1.5,
                        "octaves": 2,
                        "seed": 0,
                        "speed": 0.5,
                        "tilt": 35,
                        "roll": 0,
                        "height": 0,
                        "zoom": 1,
                        "nearCutoff": 0,
                        "farCutoff": 1,
                        "edgePinning": 0,
                        "edges": "mirror",
                        "lighting": 30,
                        "glossiness": 0,
                        "highlights": 15,
                        "lightX": 0.4,
                        "lightY": -0.6,
                        "lightZ": 0.7,
                        "lightColor": "#ffffff",
                        "cursorIntensity": 1,
                        "cursorSpeed": 0.5
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-surface3d-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "TiltShift": {
        frames: 1,
        anim: false,
        components: [{
                "type": "TiltShift",
                "id": "s8-tiltshift",
                "props": {
                        "intensity": 50,
                        "width": 0.3,
                        "falloff": 0.3,
                        "angle": 0,
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "dispersion": 0,
                        "jitter": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-tiltshift-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "TimeTrail": {
        frames: 1,
        anim: false,
        components: [{
                "type": "TimeTrail",
                "id": "s8-timetrail",
                "props": {
                        "trailSource": "motion",
                        "motionThreshold": 0.06,
                        "trailLength": 0.6,
                        "diffusion": 1,
                        "trailOpacity": 1,
                        "trailBlend": "normal",
                        "driftX": 0,
                        "driftY": 0,
                        "zoom": 1,
                        "tintMode": "none",
                        "tint": "#33aaff",
                        "speed": 1
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-timetrail-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Tint": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Tint",
                "id": "s8-tint",
                "props": {
                        "color": "#ff8800",
                        "amount": 0.5,
                        "preserveLuminosity": true
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-tint-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Tritone": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Tritone",
                "id": "s8-tritone",
                "props": {
                        "colorA": "#ce1bea",
                        "colorB": "#2fff00",
                        "colorC": "#ffff00",
                        "blendMid": 0.5,
                        "colorSpace": "linear"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-tritone-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Twirl": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Twirl",
                "id": "s8-twirl",
                "props": {
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "intensity": 1,
                        "edges": "stretch"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-twirl-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "VHS": {
        frames: 1,
        anim: false,
        components: [{
                "type": "VHS",
                "id": "s8-vhs",
                "props": {
                        "wobble": 1,
                        "scanlineNoise": 0.6,
                        "smear": 0.2,
                        "speed": 1
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-vhs-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Vibrance": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Vibrance",
                "id": "s8-vibrance",
                "props": {
                        "intensity": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-vibrance-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Vignette": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Vignette",
                "id": "s8-vignette",
                "props": {
                        "color": "#000000",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "radius": 0.5,
                        "falloff": 0.5,
                        "intensity": 1
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-vignette-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Watercolor": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Watercolor",
                "id": "s8-watercolor",
                "props": {
                        "radius": 3,
                        "bleed": 1,
                        "strength": 1,
                        "paper": 0.35,
                        "paperColor": "#fbf7ec"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-watercolor-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "WaveDistortion": {
        frames: 1,
        anim: false,
        components: [{
                "type": "WaveDistortion",
                "id": "s8-wavedistortion",
                "props": {
                        "strength": 0.3,
                        "frequency": 1,
                        "speed": 1,
                        "angle": 0,
                        "waveType": "sine",
                        "edges": "stretch"
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-wavedistortion-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "Wool": {
        frames: 1,
        anim: false,
        components: [{
                "type": "Wool",
                "id": "s8-wool",
                "props": {
                        "intensity": 0.5,
                        "scale": 4,
                        "contrast": -0.5,
                        "distortion": 0.15,
                        "seed": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-wool-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    "ZoomBlur": {
        frames: 1,
        anim: false,
        components: [{
                "type": "ZoomBlur",
                "id": "s8-zoomblur",
                "props": {
                        "intensity": 30,
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "dispersion": 0,
                        "falloff": 0,
                        "focus": 0,
                        "bias": 0,
                        "highlights": 0,
                        "spiral": 0,
                        "jitter": 0
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s8-zoomblur-c",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        }
                ]
        }],
    },
    // ── 批次 S9：Transitions 13 案（双 child，progress=0.5 中间态）（props=registry default；散列出列见 gen-presets-batch.mjs）──
    "BarnDoors": {
        frames: 1,
        anim: false,
        components: [{
                "type": "BarnDoors",
                "id": "s9-barndoors",
                "props": {
                        "progress": 0.5,
                        "angle": 0,
                        "feather": 0.1,
                        "invert": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-barndoors-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-barndoors-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },
    "BlockDissolve": {
        frames: 1,
        anim: false,
        components: [{
                "type": "BlockDissolve",
                "id": "s9-blockdissolve",
                "props": {
                        "progress": 0.5,
                        "blockSize": 0.08,
                        "softness": 0.15,
                        "invert": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-blockdissolve-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-blockdissolve-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },
    "CheckerWipe": {
        frames: 1,
        anim: false,
        components: [{
                "type": "CheckerWipe",
                "id": "s9-checkerwipe",
                "props": {
                        "progress": 0.5,
                        "blockSize": 0.1,
                        "softness": 0.15,
                        "invert": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-checkerwipe-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-checkerwipe-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },
    "DiamondWipe": {
        frames: 1,
        anim: false,
        components: [{
                "type": "DiamondWipe",
                "id": "s9-diamondwipe",
                "props": {
                        "progress": 0.5,
                        "size": 0.15,
                        "feather": 0.1,
                        "invert": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-diamondwipe-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-diamondwipe-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },
    "IrisWipe": {
        frames: 1,
        anim: false,
        components: [{
                "type": "IrisWipe",
                "id": "s9-iriswipe",
                "props": {
                        "progress": 0.5,
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "feather": 0.1,
                        "invert": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-iriswipe-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-iriswipe-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },
    "LinearWipe": {
        frames: 1,
        anim: false,
        components: [{
                "type": "LinearWipe",
                "id": "s9-linearwipe",
                "props": {
                        "progress": 0.5,
                        "angle": 0,
                        "feather": 0.1,
                        "invert": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-linearwipe-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-linearwipe-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },
    "NoiseDissolve": {
        frames: 1,
        anim: false,
        components: [{
                "type": "NoiseDissolve",
                "id": "s9-noisedissolve",
                "props": {
                        "progress": 0.5,
                        "scale": 3,
                        "softness": 0.25,
                        "seed": 0,
                        "invert": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-noisedissolve-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-noisedissolve-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },
    "PagePeel": {
        frames: 1,
        anim: false,
        components: [{
                "type": "PagePeel",
                "id": "s9-pagepeel",
                "props": {
                        "corner": "bottom-right",
                        "amount": 0.2,
                        "radius": 0.2,
                        "shading": 0.55,
                        "highlight": 0.4,
                        "highlightSoftness": 0.2,
                        "shadow": 1
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-pagepeel-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-pagepeel-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },
    "RadialWipe": {
        frames: 1,
        anim: false,
        components: [{
                "type": "RadialWipe",
                "id": "s9-radialwipe",
                "props": {
                        "progress": 0.5,
                        "startAngle": 0,
                        "direction": "cw",
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "feather": 0.1,
                        "invert": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-radialwipe-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-radialwipe-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },
    "RandomBars": {
        frames: 1,
        anim: false,
        components: [{
                "type": "RandomBars",
                "id": "s9-randombars",
                "props": {
                        "progress": 0.5,
                        "angle": 0,
                        "barCount": 12,
                        "softness": 0.15,
                        "invert": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-randombars-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-randombars-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },
    "RippleWipe": {
        frames: 1,
        anim: false,
        components: [{
                "type": "RippleWipe",
                "id": "s9-ripplewipe",
                "props": {
                        "progress": 0.5,
                        "center": {
                                "x": 0.5,
                                "y": 0.5
                        },
                        "rings": 8,
                        "feather": 0.2,
                        "invert": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-ripplewipe-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-ripplewipe-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },
    "SliceWipe": {
        frames: 1,
        anim: false,
        components: [{
                "type": "SliceWipe",
                "id": "s9-slicewipe",
                "props": {
                        "progress": 0.5,
                        "angle": 0,
                        "sliceCount": 8
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-slicewipe-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-slicewipe-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },
    "VenetianBlinds": {
        frames: 1,
        anim: false,
        components: [{
                "type": "VenetianBlinds",
                "id": "s9-venetianblinds",
                "props": {
                        "progress": 0.5,
                        "angle": 0,
                        "stripCount": 5,
                        "feather": 0.15,
                        "invert": false
                },
                "children": [
                        {
                                "type": "LinearGradient",
                                "id": "s9-venetianblinds-c1",
                                "props": {
                                        "colorA": "#0f172a",
                                        "colorB": "#7c3aed",
                                        "angle": 45
                                }
                        },
                        {
                                "type": "RadialGradient",
                                "id": "s9-venetianblinds-c2",
                                "props": {
                                        "colorA": "#0ea5e9",
                                        "colorB": "#f97316",
                                        "repeat": 2
                                }
                        }
                ]
        }],
    },

};
