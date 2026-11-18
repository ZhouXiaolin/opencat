// HyperFrames 私有扩展层（core 内，但不属于 HTML 标准、也不属于共有扩展）。
//
// 共有机制（data-composition-*、可见时间窗、data-media-start、data-volume、
// data-color-matrix、data-hide-after-end、__timelines 注册表、通用事件行为
// createEventTarget）定义在 `shared_api.js`。本文件只叠加 HF 私有的
// data-track-index 属性与 hf-seek 播放器事件（名字 + 每帧派发入口），
// 并把共有面通过 `__ocHyperframes` 原样暴露，兼容既有消费方。
//
// hf-seek 事件不做监听隔离：页面 `window.addEventListener('hf-seek', …)`
// 落进标准 window 事件面（shared 层 createEventTarget），派发也走同一面。
// 历史上的专用监听表（__ocHfListeners）无人派发、监听永不触发，已退役。
(function () {
    'use strict';
    if (globalThis.__ocHyperframes) return;

    var shared = globalThis.__ocShared;
    if (!shared) throw new Error('shared_api.js must load before hyperframes_api.js');
    // HF 私有键名与事件名从生成词表取，本层不写字面量。
    var vocab = globalThis.__ocVocab;
    if (!vocab) throw new Error('protocol-vocab.generated.js must load before hyperframes_api.js');

    // HF 独有：播放器 seek 事件。frame=f(t) 驱动每帧在 rAF flush 后按虚拟
    // 钟派发一次（realm.rs 调 dispatchSeek）；liquid 等 WebGL 自绘语料把整帧
    // 重绘挂在这个事件上，它们没有自己的 rAF 循环。
    var SEEK_EVENT = vocab.events.SEEK;

    // 每帧派发入口：构造事件并经标准 window 事件面派发。单监听异常由共享层
    // 留痕（__ocLogs），这里只兜装配面缺失等自身错误。
    function dispatchSeek(timeSecs) {
        try {
            globalThis.dispatchEvent(new CustomEvent(SEEK_EVENT, { detail: { time: timeSecs } }));
        } catch (error) {
            (globalThis.__ocLogs = globalThis.__ocLogs || []).push(
                SEEK_EVENT + ' handler threw: ' + (error && (error.stack || error))
            );
        }
    }

    // opencat 特色 `<transition>` 节点的往返序列化：from/to/effect 是 serde
    // 必填字段（缺了整份 extract 硬失败），必须无条件回传；可选字段缺省
    // 不发。脚本建元从 __attrs 取（data-*），seed 物化的 payload 带 serde
    // 字段名——两条路同一语义，缺省与静态 parse 一致（'' / '' / 'cut'）。
    function serializeTransition(el, payload, out) {
        var attrs = el && el.__attrs ? el.__attrs : {};
        // duration 走共有词表（data-duration），其余协议键是 HF 私有。
        var hf = attr;
        var pick = function (key, payloadKey) {
            var raw = attrs[key] !== undefined ? attrs[key] : payload[payloadKey];
            return raw === undefined || raw === null ? '' : String(raw);
        };
        var optNum = function (key, payloadKey) {
            var raw = pick(key, payloadKey);
            if (raw === '') return undefined;
            var n = parseFloat(raw);
            return isFinite(n) ? n : undefined;
        };
        var optStr = function (key, payloadKey) {
            var raw = pick(key, payloadKey);
            return raw === '' ? undefined : raw;
        };
        out.from = pick(hf.FROM, 'from');
        out.to = pick(hf.TO, 'to');
        out.effect = pick(hf.EFFECT, 'effect') || 'cut';
        if (out.duration === undefined) {
            var d = optNum(hf.DURATION, 'duration');
            if (d !== undefined) out.duration = d;
        }
        var v;
        if ((v = optStr(hf.DIRECTION, 'direction')) !== undefined) out.direction = v;
        if ((v = optStr(hf.TIMING, 'timing')) !== undefined) out.timing = v;
        if ((v = optNum(hf.DAMPING, 'damping')) !== undefined) out.damping = v;
        if ((v = optNum(hf.STIFFNESS, 'stiffness')) !== undefined) out.stiffness = v;
        if ((v = optNum(hf.MASS, 'mass')) !== undefined) out.mass = v;
        if ((v = optNum(hf.SEED, 'seed')) !== undefined) out.seed = v;
        if ((v = optNum(hf.HUE_SHIFT, 'hueShift')) !== undefined) out.hueShift = v;
        if ((v = optNum(hf.MASK_SCALE, 'maskScale')) !== undefined) out.maskScale = v;
    }

    var attr = Object.freeze(Object.assign({}, shared.attr, vocab.hfAttr));

    globalThis.__ocHyperframes = Object.assign({}, shared, {
        attr: attr,
        dispatchSeek: dispatchSeek,
        serializeTransition: serializeTransition,
    });
})();
