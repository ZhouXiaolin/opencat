// HyperFrames 扩展层（core 内，但不属于 HTML 标准）。
//
// 这里只放框架私有协议：`data-composition-*`、可见时间窗、`data-media-start`、
// `data-color-matrix`、`data-hide-after-end`、播放器 `hf-seek` 事件、
// `window.__timelines` 注册表和 composition 作用域钩子。
//
// core 是确定性视频渲染器，不是浏览器内核。标准 HTML DOM 输入语义由
// `html_std_api.js` 解释；本扩展层通过 `globalThis.__ocHyperframes` 把
// 组合协议挂进标准层，不复制标准实现。
(function () {
    'use strict';
    if (globalThis.__ocHyperframes) return;

    var attr = Object.freeze({
        COMPOSITION_ID: 'data-composition-id',
        COMPOSITION_SRC: 'data-composition-src',
        START: 'data-start',
        DURATION: 'data-duration',
        WIDTH: 'data-width',
        HEIGHT: 'data-height',
        FPS: 'data-fps',
        MEDIA_START: 'data-media-start',
        COLOR_MATRIX: 'data-color-matrix',
        HIDE_AFTER_END: 'data-hide-after-end',
        SEEK_EVENT: 'hf-seek',
    });

    var protocolField = Object.freeze({
        START: 'data_start',
        DURATION: 'data_duration',
    });

    function readProtocol(node) {
        var attrs = node && node.attrs ? node.attrs : {};
        return {
            compositionId: attrs[attr.COMPOSITION_ID],
            compositionSrc: attrs[attr.COMPOSITION_SRC],
            start: parseFloatAttr(attrs[attr.START]),
            duration: parseFloatAttr(attrs[attr.DURATION]),
            width: parseFloatAttr(attrs[attr.WIDTH]),
            height: parseFloatAttr(attrs[attr.HEIGHT]),
            fps: parseFloatAttr(attrs[attr.FPS]),
            mediaStart: parseFloatAttr(attrs[attr.MEDIA_START]),
            colorMatrix: attrs[attr.COLOR_MATRIX],
            hideAfterEnd: attrs[attr.HIDE_AFTER_END] === undefined
                ? undefined
                : attrs[attr.HIDE_AFTER_END] === '' || attrs[attr.HIDE_AFTER_END] === 'true',
        };
    }

    function parseFloatAttr(value) {
        if (value === undefined || value === null || value === '') return undefined;
        var n = parseFloat(value);
        return isFinite(n) ? n : undefined;
    }

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
    }

    function installTimelinesRegistry(windowObject) {
        if (!windowObject || typeof windowObject !== 'object') return;
        windowObject.__timelines = windowObject.__timelines || {};
    }

    function installSeekDispatcher(windowObject) {
        if (!windowObject || typeof windowObject !== 'object' || windowObject.__ocHfSeekInstalled) return;
        windowObject.__ocHfSeekInstalled = true;
        var add = windowObject.addEventListener;
        var remove = windowObject.removeEventListener;
        var dispatch = windowObject.dispatchEvent;
        var listeners = windowObject.__ocHfListeners || (windowObject.__ocHfListeners = {});
        windowObject.addEventListener = function (type, fn) {
            if (type === attr.SEEK_EVENT && typeof fn === 'function') {
                (listeners[type] = listeners[type] || []).push(fn);
                return;
            }
            if (typeof add === 'function') add.call(this, type, fn);
        };
        windowObject.removeEventListener = function (type, fn) {
            if (type === attr.SEEK_EVENT) {
                var list = listeners[type];
                if (list) {
                    var index = list.indexOf(fn);
                    if (index >= 0) list.splice(index, 1);
                }
                return;
            }
            if (typeof remove === 'function') remove.call(this, type, fn);
        };
        windowObject.dispatchEvent = function (ev) {
            var list = ev && listeners[ev.type];
            if (list) {
                list.slice().forEach(function (fn) {
                    try { fn.call(windowObject, ev); }
                    catch (error) {
                        (globalThis.__ocLogs = globalThis.__ocLogs || []).push(
                            attr.SEEK_EVENT + ' handler threw: ' + (error && (error.stack || error))
                        );
                    }
                });
            }
            if ((!ev || ev.type !== attr.SEEK_EVENT) && typeof dispatch === 'function') {
                return dispatch.call(this, ev);
            }
            return true;
        };
    }

    function seekEventJs() {
        return "if (globalThis.__ocMode !== 'extract') { try { globalThis.dispatchEvent(new CustomEvent('" +
            attr.SEEK_EVENT +
            "', { detail: { time: (globalThis.__ocClockMs() || 0) / 1000 } })); } catch (error) { (globalThis.__ocLogs = globalThis.__ocLogs || []).push('" +
            attr.SEEK_EVENT +
            " handler threw: ' + (error && (error.stack || error))); } } }";
    }

    globalThis.__ocHyperframes = {
        attr: attr,
        protocolField: protocolField,
        readProtocol: readProtocol,
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
        installTimelinesRegistry: function (windowObject) {
            installTimelinesRegistry(windowObject);
            installSeekDispatcher(windowObject);
        },
        seekEventJs: seekEventJs,
    };
})();
