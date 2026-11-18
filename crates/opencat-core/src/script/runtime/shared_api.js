// 共有扩展层：HyperFrames 与 opencat 都要用的组合协议机制。
//
// 标准 HTML DOM 语义由 `html_std_api.js` 解释；本层只定义两边共有的
// `data-*` 词汇表与 `window.__timelines` 注册表机制。各框架私有的
// 事件名与属性在各自的 api.js 文件中叠加。
(function () {
    'use strict';
    if (globalThis.__ocShared) return;

    // data-* 键名从生成词表取（manifest 第一段注入），本层不写协议字面量。
    var vocab = globalThis.__ocVocab;
    if (!vocab) throw new Error('protocol-vocab.generated.js must load before shared_api.js');

    var attr = vocab.attr;

    var protocolField = Object.freeze({
        START: 'data_start',
        DURATION: 'data_duration',
    });

    function serializeVideo(el, payload, out) {
        var attrs = el && el.__attrs ? el.__attrs : {};
        var mediaStart = attrs[attr.MEDIA_START] !== undefined
            ? parseFloat(attrs[attr.MEDIA_START])
            : payload[attr.MEDIA_START];
        if (typeof mediaStart === 'number' && !isNaN(mediaStart)) out[attr.MEDIA_START] = mediaStart;
        if (payload[attr.COLOR_MATRIX] !== undefined && payload[attr.COLOR_MATRIX] !== null) {
            out[attr.COLOR_MATRIX] = payload[attr.COLOR_MATRIX];
        }
        if (payload[attr.HIDE_AFTER_END] !== undefined && payload[attr.HIDE_AFTER_END] !== null) {
            out[attr.HIDE_AFTER_END] = payload[attr.HIDE_AFTER_END];
        }
        // 元素增益（语料 `<audio data-volume="0.30">`）：脚本建元从 __attrs
        // 取，seed 物化的 payload 带 data-volume 键。
        var volume = attrs[attr.VOLUME] !== undefined
            ? parseFloat(attrs[attr.VOLUME])
            : payload[attr.VOLUME];
        if (typeof volume === 'number' && !isNaN(volume)) out[attr.VOLUME] = volume;
    }

    function installTimelinesRegistry(windowObject) {
        if (!windowObject || typeof windowObject !== 'object') return;
        windowObject.__timelines = windowObject.__timelines || {};
    }

    // 通用事件行为：监听表 + 派发。标准 window 事件面（dom_api）与扩展事件
    // 都走这一份实现，各处不再自维护表——注册/派发语义（快照迭代、单监听
    // 异常不拖垮派发且留痕）单点收敛在这里。
    function createEventTarget(host) {
        var listeners = {};
        return {
            addEventListener: function (type, fn) {
                if (typeof fn !== 'function') return;
                (listeners[String(type)] = listeners[String(type)] || []).push(fn);
            },
            removeEventListener: function (type, fn) {
                var a = listeners[String(type)];
                if (a) {
                    var i = a.indexOf(fn);
                    if (i >= 0) a.splice(i, 1);
                }
            },
            dispatchEvent: function (ev) {
                var a = ev && listeners[ev.type];
                if (a) {
                    a.slice().forEach(function (fn) {
                        try {
                            fn.call(host, ev);
                        } catch (e) {
                            // 单监听失败不拖垮派发，但必须留下痕迹，避免逐帧
                            // 重绘静默失效时无据可查。
                            try {
                                (globalThis.__ocLogs = globalThis.__ocLogs || []).push(
                                    'event handler ' + ev.type + ' threw: ' + (e && e.stack || e)
                                );
                            } catch (e2) { /* 日志通道也不可用时放弃 */ }
                        }
                    });
                }
                return true;
            },
        };
    }

    globalThis.__ocShared = {
        attr: attr,
        protocolField: protocolField,
        createEventTarget: createEventTarget,
        isProtocolAttr: function (name) {
            return name === attr.START || name === attr.DURATION;
        },
        readSeedProtocol: function (payload) {
            if (!payload) return payload;
            if (payload[protocolField.START] === undefined && payload[attr.START] !== undefined) {
                var start = parseFloat(payload[attr.START]);
                if (isFinite(start)) payload[protocolField.START] = start;
            }
            if (payload[protocolField.DURATION] === undefined && payload[attr.DURATION] !== undefined) {
                var duration = parseFloat(payload[attr.DURATION]);
                if (isFinite(duration)) payload[protocolField.DURATION] = duration;
            }
            return payload;
        },
        readElementAttr: function (name, value) {
            if (name === attr.START) return { [protocolField.START]: parseFloat(value) };
            if (name === attr.DURATION) return { [protocolField.DURATION]: parseFloat(value) };
            return null;
        },
        getElementAttr: function (payload, name) {
            if (name === attr.START) {
                return payload[protocolField.START] === undefined
                    ? null
                    : String(payload[protocolField.START]);
            }
            if (name === attr.DURATION) {
                return payload[protocolField.DURATION] === undefined
                    ? null
                    : String(payload[protocolField.DURATION]);
            }
            return undefined;
        },
        writeSerializeAttrs: function (payload, out) {
            if (payload[protocolField.START] !== undefined && payload[protocolField.START] !== null) {
                out[attr.START] = payload[protocolField.START];
            }
            if (payload[protocolField.DURATION] !== undefined && payload[protocolField.DURATION] !== null) {
                out[attr.DURATION] = payload[protocolField.DURATION];
            }
        },
        serializeVideo: serializeVideo,
        installTimelinesRegistry: installTimelinesRegistry,
    };
})();
