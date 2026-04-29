(function() {
    var SPRING_PRESETS = {
        'spring.default':  'spring:100,10,1',
        'spring.gentle':   'spring:60,8,0.8',
        'spring.stiff':    'spring:200,15,1',
        'spring.slow':     'spring:80,12,1.5',
        'spring.wobbly':   'spring:180,6,1',
        'spring-default':  'spring:100,10,1',
        'spring-gentle':   'spring:60,8,0.8',
        'spring-stiff':    'spring:200,15,1',
        'spring-slow':     'spring:80,12,1.5',
        'spring-wobbly':   'spring:180,6,1',
    };

    var RESERVED = {
        duration: true,
        delay: true,
        ease: true,
        easing: true,
        clamp: true,
        repeat: true,
        yoyo: true,
        repeatDelay: true,
        stagger: true,
        keyframes: true,
        path: true,
        orient: true,
        at: true,
        mode: true,
        textMode: true,
    };

    var DEFAULTS = {
        opacity: 1,
        x: 0,
        y: 0,
        translateX: 0,
        translateY: 0,
        scale: 1,
        scaleX: 1,
        scaleY: 1,
        rotate: 0,
        rotation: 0,
        skewX: 0,
        skewY: 0,
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
        width: 0,
        height: 0,
        borderRadius: 0,
        borderWidth: 0,
        strokeWidth: 0,
    };

    var SETTER = {
        opacity: 'opacity',
        x: 'translateX',
        y: 'translateY',
        translateX: 'translateX',
        translateY: 'translateY',
        scale: 'scale',
        scaleX: 'scaleX',
        scaleY: 'scaleY',
        rotate: 'rotate',
        rotation: 'rotate',
        skewX: 'skewX',
        skewY: 'skewY',
        left: 'left',
        top: 'top',
        right: 'right',
        bottom: 'bottom',
        width: 'width',
        height: 'height',
        backgroundColor: 'bg',
        bg: 'bg',
        color: 'textColor',
        textColor: 'textColor',
        borderRadius: 'borderRadius',
        borderWidth: 'borderWidth',
        borderColor: 'borderColor',
        fillColor: 'fillColor',
        strokeColor: 'strokeColor',
        strokeWidth: 'strokeWidth',
        textSize: 'textSize',
        letterSpacing: 'letterSpacing',
        lineHeight: 'lineHeight',
        text: 'text',
    };

    function hasOwn(obj, key) {
        return Object.prototype.hasOwnProperty.call(obj, key);
    }

    function resolveEasingTag(easing) {
        if (easing == null) {
            return 'linear';
        }
        if (typeof easing === 'string') {
            if (SPRING_PRESETS[easing]) {
                return SPRING_PRESETS[easing];
            }
            return easing;
        }
        if (Array.isArray(easing) && easing.length === 4) {
            return 'bezier:' + easing[0] + ',' + easing[1] + ',' + easing[2] + ',' + easing[3];
        }
        if (easing && typeof easing === 'object' && easing.spring) {
            var s = easing.spring;
            return 'spring:' + s.stiffness + ',' + s.damping + ',' + s.mass;
        }
        throw new Error('invalid ease value');
    }

    function normalizeSelector(target) {
        if (typeof target !== 'string') {
            return target;
        }
        return target.charAt(0) === '#' ? target.slice(1) : target;
    }

    function normalizeTargets(targets) {
        if (targets == null) {
            return [];
        }
        if (typeof targets === 'string') {
            return [normalizeSelector(targets)];
        }
        if (Array.isArray(targets)) {
            return targets.slice().map(normalizeSelector);
        }
        if (typeof targets.set === 'function') {
            return [targets];
        }
        throw new Error('target must be an id string, array, or splitText part');
    }

    function animKeys(vars) {
        var keys = [];
        vars = vars || {};
        for (var key in vars) {
            if (hasOwn(vars, key) && !RESERVED[key]) {
                if (!SETTER[key]) {
                    throw new Error('unsupported animation property `' + key + '`');
                }
                keys.push(key);
            }
        }
        return keys;
    }

    function mergeVars(base, extra) {
        var out = {};
        base = base || {};
        extra = extra || {};
        for (var k in base) {
            if (hasOwn(base, k)) out[k] = base[k];
        }
        for (var e in extra) {
            if (hasOwn(extra, e)) out[e] = extra[e];
        }
        return out;
    }

    function inferFromValue(key, toValue) {
        if (key === 'text') {
            return '';
        }
        if (hasOwn(DEFAULTS, key)) {
            return DEFAULTS[key];
        }
        if (typeof toValue === 'number') {
            return 0;
        }
        return toValue;
    }

    function normalizeKeyframes(spec) {
        if (!Array.isArray(spec)) {
            throw new Error('keyframes value must be an array');
        }
        if (spec.length === 0) {
            throw new Error('keyframes array cannot be empty');
        }
        var first = spec[0];
        var isShorthand = typeof first === 'number';
        var normalized;
        if (isShorthand) {
            var n = spec.length;
            normalized = [];
            for (var i = 0; i < n; i++) {
                normalized.push({
                    at: n === 1 ? 0 : i / (n - 1),
                    value: Number(spec[i]),
                    easing: null,
                });
            }
        } else {
            normalized = spec.map(function(kf) {
                if (kf == null || typeof kf.at !== 'number' || typeof kf.value !== 'number') {
                    throw new Error('keyframe entry requires numeric `at` and `value`');
                }
                return {
                    at: kf.at,
                    value: Number(kf.value),
                    easing: kf.easing != null ? resolveEasingTag(kf.easing) : null,
                };
            });
            normalized.sort(function(a, b) { return a.at - b.at; });
        }
        return normalized;
    }

    function evaluateKeyframes(progress, kfs) {
        var p = Math.max(0, Math.min(1, Number(progress)));
        if (p <= kfs[0].at) return kfs[0].value;
        var last = kfs[kfs.length - 1];
        if (p >= last.at) return last.value;
        for (var i = 0; i < kfs.length - 1; i++) {
            var a = kfs[i];
            var b = kfs[i + 1];
            if (p >= a.at && p <= b.at) {
                var span = b.at - a.at;
                var localT = span > 0 ? (p - a.at) / span : 0;
                if (b.easing) {
                    localT = __easing_apply(b.easing, localT);
                }
                return a.value + (b.value - a.value) * localT;
            }
        }
        return last.value;
    }

    function createTween(fromVars, toVars, timing) {
        timing = timing || {};
        fromVars = fromVars || {};
        toVars = toVars || {};
        var easingTag = resolveEasingTag(timing.ease != null ? timing.ease : timing.easing);
        var isSpring = easingTag.indexOf('spring:') === 0;
        var duration = timing.duration;
        if (duration === undefined && !isSpring) {
            throw new Error('duration is required for non-spring tweens');
        }

        var handle = __animate_create(
            duration !== undefined ? Number(duration) : -1,
            Number(timing.delay || 0),
            timing.clamp === false ? 0 : 1,
            easingTag,
            timing.repeat !== undefined ? Number(timing.repeat) : 0,
            timing.yoyo ? 1 : 0,
            timing.repeatDelay !== undefined ? Number(timing.repeatDelay) : 0
        );

        var keys = animKeys(toVars);
        var fromKeys = animKeys(fromVars);
        for (var fi = 0; fi < fromKeys.length; fi++) {
            if (keys.indexOf(fromKeys[fi]) === -1) {
                keys.push(fromKeys[fi]);
            }
        }

        var keyframesSpec = timing.keyframes || toVars.keyframes || null;
        var keyframesNormalized = null;
        if (keyframesSpec) {
            keyframesNormalized = {};
            for (var kfKey in keyframesSpec) {
                if (hasOwn(keyframesSpec, kfKey)) {
                    keyframesNormalized[kfKey] = normalizeKeyframes(keyframesSpec[kfKey]);
                    if (keys.indexOf(kfKey) === -1) {
                        keys.push(kfKey);
                    }
                }
            }
        }

        var pathHandle = null;
        var pathOrient = 0;
        if (timing.path || toVars.path) {
            var svg = String(timing.path || toVars.path);
            pathOrient = timing.orient !== undefined ? Number(timing.orient) : Number(toVars.orient || 0);
            var cacheKey = '__ap_' + svg;
            if (!ctx[cacheKey]) {
                ctx[cacheKey] = __along_path_create(svg);
            }
            pathHandle = ctx[cacheKey];
            if (keys.indexOf('x') === -1) keys.push('x');
            if (keys.indexOf('y') === -1) keys.push('y');
            if (keys.indexOf('rotation') === -1) keys.push('rotation');
        }

        function pathSample() {
            var p = __animate_progress(handle);
            return __along_path_at(pathHandle, p);
        }

        var result = {
            progress: __animate_progress(handle),
            settled: __animate_settled(handle),
            settleFrame: __animate_settle_frame(handle),
            values: {},
        };

        for (var i = 0; i < keys.length; i++) {
            var key = keys[i];
            if (keyframesNormalized && keyframesNormalized[key]) {
                result.values[key] = evaluateKeyframes(result.progress, keyframesNormalized[key]);
                continue;
            }
            if (pathHandle != null && (key === 'x' || key === 'y' || key === 'rotation')) {
                var sample = pathSample();
                result.values[key] = key === 'x' ? sample[0] : (key === 'y' ? sample[1] : sample[2] + pathOrient);
                continue;
            }
            var toVal = hasOwn(toVars, key) ? toVars[key] : inferFromValue(key, fromVars[key]);
            var fromVal = hasOwn(fromVars, key) ? fromVars[key] : inferFromValue(key, toVal);
            if (key === 'text') {
                result.values[key] = sampleText(fromVal, toVal, result.progress, timing.mode || timing.textMode || toVars.mode || toVars.textMode);
            } else if (typeof fromVal === 'string' || typeof toVal === 'string') {
                result.values[key] = __animate_color(handle, key, String(fromVal), String(toVal));
            } else {
                result.values[key] = __animate_value(handle, key, Number(fromVal), Number(toVal));
            }
            result[key] = result.values[key];
        }

        return result;
    }

    function sampleText(fromText, toText, progress, mode) {
        var from = fromText == null ? '' : String(fromText);
        var to = toText == null ? '' : String(toText);
        var textMode = mode || 'typewriter';
        if (progress <= 0) return from;
        if (progress >= 1) return to;
        if (textMode !== 'typewriter') {
            throw new Error('unsupported text animation mode `' + textMode + '`');
        }
        var parts = __text_graphemes(to);
        var n = Math.floor(parts.length * progress);
        return parts.slice(0, n).join('');
    }

    function applyValues(target, values) {
        if (typeof target === 'string') {
            var node = ctx.getNode(target);
            for (var key in values) {
                if (!hasOwn(values, key)) continue;
                var setter = SETTER[key];
                if (!setter) {
                    throw new Error('unsupported animation property `' + key + '`');
                }
                node[setter](values[key]);
            }
            return;
        }
        if (target && typeof target.set === 'function') {
            target.set(values);
            return;
        }
        throw new Error('invalid target');
    }

    function applyTween(targets, fromVars, toVars, timing) {
        var list = normalizeTargets(targets);
        var stagger = timing && timing.stagger !== undefined ? Number(timing.stagger) : 0;
        var results = [];
        for (var i = 0; i < list.length; i++) {
            var localTiming = mergeVars(timing, {
                delay: Number((timing && timing.delay) || 0) + i * stagger,
            });
            var tween = createTween(fromVars, toVars, localTiming);
            applyValues(list[i], tween.values);
            results.push(tween);
        }
        return list.length === 1 ? results[0] : results;
    }

    function splitTiming(vars, extraDelay) {
        var timing = {};
        vars = vars || {};
        for (var key in vars) {
            if (hasOwn(vars, key) && RESERVED[key]) {
                timing[key] = vars[key];
            }
        }
        if (vars.ease !== undefined && timing.easing === undefined) {
            timing.easing = vars.ease;
        }
        if (extraDelay !== undefined) {
            timing.delay = Number(timing.delay || 0) + Number(extraDelay);
        }
        return timing;
    }

    ctx.set = function(targets, vars) {
        var list = normalizeTargets(targets);
        var values = {};
        var keys = animKeys(vars);
        for (var i = 0; i < keys.length; i++) {
            values[keys[i]] = vars[keys[i]];
        }
        for (var ti = 0; ti < list.length; ti++) {
            applyValues(list[ti], values);
        }
        return list;
    };

    ctx.to = function(targets, vars) {
        return applyTween(targets, {}, vars || {}, splitTiming(vars));
    };

    ctx.from = function(targets, vars) {
        var toVars = {};
        var keys = animKeys(vars || {});
        for (var i = 0; i < keys.length; i++) {
            toVars[keys[i]] = inferFromValue(keys[i], vars[keys[i]]);
        }
        return applyTween(targets, vars || {}, toVars, splitTiming(vars));
    };

    ctx.fromTo = function(targets, fromVars, toVars) {
        return applyTween(targets, fromVars || {}, toVars || {}, splitTiming(toVars));
    };

    function parsePosition(pos, cursor, labels) {
        if (pos == null) return cursor;
        if (typeof pos === 'number') return pos;
        if (typeof pos === 'string') {
            if (hasOwn(labels, pos)) return labels[pos];
            if (pos.indexOf('+=') === 0) return cursor + Number(pos.slice(2));
            if (pos.indexOf('-=') === 0) return cursor - Number(pos.slice(2));
            var n = Number(pos);
            if (!isNaN(n)) return n;
        }
        throw new Error('unsupported timeline position `' + pos + '`');
    }

    ctx.timeline = function(opts) {
        opts = opts || {};
        var baseDelay = Number(opts.delay || 0);
        var defaults = opts.defaults || {};
        var labels = {};
        var cursor = 0;

        function addTween(kind, targets, a, b, pos) {
            var fromVars;
            var toVars;
            if (kind === 'fromTo') {
                fromVars = a || {};
                toVars = b || {};
            } else if (kind === 'from') {
                fromVars = a || {};
                toVars = {};
                var fkeys = animKeys(fromVars);
                for (var fi = 0; fi < fkeys.length; fi++) {
                    toVars[fkeys[fi]] = inferFromValue(fkeys[fi], fromVars[fkeys[fi]]);
                }
            } else {
                fromVars = {};
                toVars = a || {};
            }

            var varsForTiming = kind === 'fromTo' ? toVars : a;
            var hasExplicitPosition = pos != null || (varsForTiming && varsForTiming.at != null);
            var start = parsePosition(hasExplicitPosition ? (pos != null ? pos : varsForTiming.at) : null, cursor, labels);
            var mergedTiming = splitTiming(mergeVars(defaults, varsForTiming), baseDelay + start);
            var result = applyTween(targets, fromVars, mergeVars(defaults, toVars), mergedTiming);
            var duration = mergedTiming.duration !== undefined ? Number(mergedTiming.duration) : 0;
            if (!hasExplicitPosition) {
                cursor = Math.max(cursor, start + duration);
            }
            return result;
        }

        var api = {
            set: function(targets, vars, pos) {
                var start = parsePosition(pos != null ? pos : vars && vars.at, cursor, labels);
                if (ctx.currentFrame >= baseDelay + start) {
                    ctx.set(targets, vars || {});
                }
                cursor = Math.max(cursor, start);
                return api;
            },
            to: function(targets, vars, pos) {
                addTween('to', targets, vars, null, pos);
                return api;
            },
            from: function(targets, vars, pos) {
                addTween('from', targets, vars, null, pos);
                return api;
            },
            fromTo: function(targets, fromVars, toVars, pos) {
                addTween('fromTo', targets, fromVars, toVars, pos);
                return api;
            },
            addLabel: function(name, pos) {
                labels[String(name)] = parsePosition(pos, cursor, labels);
                return api;
            },
        };
        return api;
    };

    ctx.splitText = function(target, opts) {
        var id = normalizeSelector(target);
        if (typeof id !== 'string' || !id) {
            throw new Error('ctx.splitText requires a node id');
        }
        var options = opts || {};
        var type = options.type || options.granularity || 'chars';
        var granularity;
        if (type === 'chars' || type === 'characters' || type === 'graphemes') {
            granularity = 'graphemes';
        } else if (type === 'words') {
            granularity = 'words';
        } else if (type === 'lines') {
            throw new Error('splitText({ type: "lines" }) requires layout-derived line ranges and is not implemented yet');
        } else {
            throw new Error('unsupported splitText type `' + type + '`');
        }

        var text = __text_source_get(id);
        if (typeof text !== 'string') {
            throw new Error('no resolved text source for node `' + id + '`');
        }

        var parts = __text_units_describe(id, granularity).map(function(meta) {
            var part = {
                index: meta[0],
                text: meta[1],
                start: meta[2],
                end: meta[3],
                set: function(values) {
                    values = values || {};
                    var mapped = {};
                    for (var key in values) {
                        if (!hasOwn(values, key)) continue;
                        switch (key) {
                            case 'opacity': mapped.opacity = values[key]; break;
                            case 'x':
                            case 'translateX': mapped.translateX = values[key]; break;
                            case 'y':
                            case 'translateY': mapped.translateY = values[key]; break;
                            case 'scale': mapped.scale = values[key]; break;
                            case 'rotate':
                            case 'rotation': mapped.rotation = values[key]; break;
                            default:
                                throw new Error('unsupported splitText property `' + key + '`');
                        }
                    }
                    __record_text_unit_override(id, granularity, meta[0], mapped);
                    return part;
                }
            };
            return part;
        });

        parts.revert = function() {
            for (var i = 0; i < parts.length; i++) {
                parts[i].set({ opacity: 1, x: 0, y: 0, scale: 1, rotate: 0 });
            }
            return parts;
        };

        return parts;
    };

    ctx.utils = {
        clamp: function(value, min, max) {
            return Math.min(max, Math.max(min, Number(value)));
        },
        snap: function(value, step) {
            var n = Number(value);
            var s = Number(step);
            if (!isFinite(s) || s <= 0) return n;
            return Math.round(n / s) * s;
        },
        wrap: function(value, min, max) {
            var lo = Number(min);
            var hi = Number(max);
            var range = hi - lo;
            if (range <= 0) return lo;
            var n = (Number(value) - lo) % range;
            if (n < 0) n += range;
            return n + lo;
        },
        mapRange: function(value, inMin, inMax, outMin, outMax) {
            var iMin = Number(inMin);
            var iMax = Number(inMax);
            if (iMax === iMin) return Number(outMin);
            var t = (Number(value) - iMin) / (iMax - iMin);
            return Number(outMin) + t * (Number(outMax) - Number(outMin));
        },
        random: function(min, max, seed) {
            var lo = Number(min);
            var hi = Number(max);
            if (seed === undefined || seed === null) {
                return lo + Math.random() * (hi - lo);
            }
            return lo + __util_random_seeded(Number(seed)) * (hi - lo);
        },
        randomInt: function(min, max, seed) {
            return Math.floor(this.random(Number(min), Number(max) + 1, seed));
        },
    };

    ctx.alongPath = function(svgPath) {
        var svg = String(svgPath);
        var handle = __along_path_create(svg);
        var len = __along_path_length(handle);
        return {
            getLength: function() { return len; },
            at: function(t) {
                if (handle == null) {
                    throw new Error('alongPath result has been disposed');
                }
                var arr = __along_path_at(handle, Number(t));
                return { x: arr[0], y: arr[1], angle: arr[2] };
            },
            dispose: function() {
                if (handle != null) {
                    __along_path_dispose(handle);
                    handle = null;
                }
            },
        };
    };
})();
