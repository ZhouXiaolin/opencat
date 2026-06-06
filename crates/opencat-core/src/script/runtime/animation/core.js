(function() {
    var runtime = globalThis.__opencatAnimation;
    var animation = runtime.animation;
    var hasOwn = runtime.hasOwn;
    var copyOwn = runtime.copyOwn;

    function resolveEasingTag(easing) {
        if (easing == null) {
            return 'linear';
        }
        if (typeof easing === 'string') {
            if (runtime.SPRING_PRESETS[easing]) {
                return runtime.SPRING_PRESETS[easing];
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

    function createTarget(raw) {
        if (typeof raw === 'string') {
            return {
                kind: 'node',
                id: raw,
                raw: raw,
                node: ctx.getNode(raw),
            };
        }
        if (raw && typeof raw.set === 'function') {
            return {
                kind: 'object',
                raw: raw,
                set: function(values) {
                    raw.set(values);
                },
            };
        }
        throw new Error('invalid target');
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

    function splitTiming(vars, extraDelay) {
        var timing = {};
        vars = vars || {};
        for (var key in vars) {
            if (hasOwn(vars, key) && animation.isTimingKey(key)) {
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

    function getDescriptorDefault(descriptor, target, key, otherValue) {
        if (typeof descriptor.inferFrom === 'function') {
            return descriptor.inferFrom(otherValue, target, key);
        }
        // Try to read current value from mutation store (includes Tailwind base style)
        if (target && target.id && typeof globalThis.__read_style_value === 'function') {
            var current = globalThis.__read_style_value(target.id, key);
            if (current != null && current !== '') {
                return current;
            }
        }
        if (hasOwn(descriptor, 'defaultValue')) {
            return typeof descriptor.defaultValue === 'function'
                ? descriptor.defaultValue(target, key, otherValue)
                : descriptor.defaultValue;
        }
        if (typeof otherValue === 'number') {
            return 0;
        }
        return otherValue;
    }

    function getVar(vars, track) {
        vars = vars || {};
        if (hasOwn(vars, track.inputName)) return vars[track.inputName];
        if (hasOwn(vars, track.name)) return vars[track.name];
        var aliases = track.descriptor.aliases || [];
        for (var i = 0; i < aliases.length; i++) {
            if (hasOwn(vars, aliases[i])) return vars[aliases[i]];
        }
        return undefined;
    }

    function resolveVarValue(value, target, targetIndex) {
        if (typeof value === 'function') {
            return value(targetIndex || 0, target ? target.raw : undefined);
        }
        return value;
    }

    function getVarForTarget(vars, track, target, targetIndex) {
        return resolveVarValue(getVar(vars, track), target, targetIndex);
    }

    function computeLinearDistances(count, from) {
        var distances = [];
        var i;
        if (typeof from === 'number') {
            for (i = 0; i < count; i++) distances.push(Math.abs(i - from));
            return distances;
        }
        switch (from || 'start') {
            case 'start':
                for (i = 0; i < count; i++) distances.push(i);
                break;
            case 'end':
                for (i = 0; i < count; i++) distances.push(count - 1 - i);
                break;
            case 'center': {
                var center = (count - 1) / 2;
                for (i = 0; i < count; i++) distances.push(Math.abs(i - center));
                break;
            }
            case 'edges':
                for (i = 0; i < count; i++) distances.push(Math.min(i, count - 1 - i));
                break;
            case 'random':
                for (i = 0; i < count; i++) {
                    var x = Math.sin((i + 1) * 12.9898 + 78.233) * 43758.5453;
                    distances.push(x - Math.floor(x));
                }
                break;
            default:
                for (i = 0; i < count; i++) distances.push(i);
        }
        return distances;
    }

    function computeGridDistances(count, grid, axis, from) {
        var cols, rows;
        if (Array.isArray(grid)) {
            rows = Number(grid[0]);
            cols = Number(grid[1]);
        } else {
            cols = typeof grid === 'number' ? Number(grid) : Math.ceil(Math.sqrt(count));
            rows = Math.ceil(count / cols);
        }

        var refRow, refCol;
        var isEdges = from === 'edges';
        if (!isEdges && typeof from === 'number') {
            refRow = Math.floor(from / cols);
            refCol = from % cols;
        } else if (!isEdges) {
            switch (from || 'start') {
                case 'center':
                    refRow = (rows - 1) / 2;
                    refCol = (cols - 1) / 2;
                    break;
                case 'end':
                    refRow = rows - 1;
                    refCol = cols - 1;
                    break;
                default:
                    refRow = 0;
                    refCol = 0;
            }
        }

        var distances = [];
        for (var i = 0; i < count; i++) {
            var row = Math.floor(i / cols);
            var col = i % cols;
            var dr, dc;
            if (isEdges) {
                dr = Math.min(row, rows - 1 - row);
                dc = Math.min(col, cols - 1 - col);
            } else {
                dr = Math.abs(row - refRow);
                dc = Math.abs(col - refCol);
            }
            var dist;
            if (axis === 'x') dist = dc;
            else if (axis === 'y') dist = dr;
            else dist = Math.sqrt(dr * dr + dc * dc);
            distances.push(dist);
        }
        return distances;
    }

    function resolveStaggerDelays(stagger, count) {
        if (count <= 1 || stagger == null || stagger === 0) return null;
        if (typeof stagger === 'number') {
            var delays = [];
            for (var i = 0; i < count; i++) delays.push(i * stagger);
            return delays;
        }
        if (typeof stagger === 'object') {
            var each = stagger.each;
            var amount = stagger.amount;

            if (each == null && amount != null) {
                each = Number(amount) / Math.max(1, count - 1);
            }
            each = Number(each || 0);

            var from = stagger.from || 'start';
            var grid = stagger.grid || null;
            var axis = stagger.axis || null;
            var distances = grid
                ? computeGridDistances(count, grid, axis, from)
                : computeLinearDistances(count, from);

            var maxDist = 0;
            for (var i = 0; i < distances.length; i++) {
                if (distances[i] > maxDist) maxDist = distances[i];
            }

            var easeTag = stagger.ease ? resolveEasingTag(stagger.ease) : null;

            var delays = [];
            for (var i = 0; i < count; i++) {
                var t = maxDist > 0 ? distances[i] / maxDist : 0;
                if (easeTag) t = __easing_apply(easeTag, t);
                delays.push(t * each * maxDist);
            }
            return delays;
        }
        return null;
    }

    function hasVar(vars, track) {
        vars = vars || {};
        if (hasOwn(vars, track.inputName) || hasOwn(vars, track.name)) return true;
        var aliases = track.descriptor.aliases || [];
        for (var i = 0; i < aliases.length; i++) {
            if (hasOwn(vars, aliases[i])) return true;
        }
        return false;
    }

    function collectTracks(fromVars, toVars, timing) {
        var tracks = [];
        var byCanonical = {};

        function add(inputName) {
            if (animation.isReservedKey(inputName)) {
                return;
            }
            var descriptor = animation.resolveProperty(inputName);
            var canonical = animation.canonicalName(inputName);
            if (!descriptor || !canonical) {
                throw new Error('unsupported animation property `' + inputName + '`');
            }
            if (byCanonical[canonical]) {
                return byCanonical[canonical];
            }
            var track = {
                inputName: inputName,
                name: canonical,
                descriptor: descriptor,
                keyframes: null,
                prepared: null,
            };
            byCanonical[canonical] = track;
            tracks.push(track);
            return track;
        }

        function addVars(vars) {
            vars = vars || {};
            for (var key in vars) {
                if (hasOwn(vars, key)) add(key);
            }
        }

        addVars(toVars);
        addVars(fromVars);

        var keyframesSpec = timing.keyframes || (toVars && toVars.keyframes) || null;
        if (keyframesSpec) {
            for (var kfKey in keyframesSpec) {
                if (hasOwn(keyframesSpec, kfKey)) {
                    add(kfKey).keyframes = normalizeKeyframes(keyframesSpec[kfKey]);
                }
            }
        }

        return {
            tracks: tracks,
            byCanonical: byCanonical,
            addTrack: add,
            hasTrack: function(name) {
                var canonical = animation.canonicalName(name) || name;
                return !!byCanonical[canonical];
            },
        };
    }

    function createTween(target, fromVars, toVars, timing, targetIndex) {
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

        var trackSet = collectTracks(fromVars, toVars, timing);
        var tweenContext = {
            target: target,
            fromVars: fromVars,
            toVars: toVars,
            timing: timing,
            handle: handle,
            tracks: trackSet.tracks,
            addTrack: trackSet.addTrack,
            hasTrack: trackSet.hasTrack,
            sampleOverrides: {},
        };

        for (var p = 0; p < animation.plugins.length; p++) {
            var plugin = animation.plugins[p];
            if (typeof plugin.augmentTween === 'function') {
                plugin.augmentTween(tweenContext);
            }
        }

        var progress = __animate_progress(handle);
        var result = {
            progress: progress,
            settled: __animate_settled(handle),
            settleFrame: __animate_settle_frame(handle),
            values: {},
        };

        for (var i = 0; i < tweenContext.tracks.length; i++) {
            var track = tweenContext.tracks[i];
            var descriptor = track.descriptor;
            var toVal = hasVar(toVars, track)
                ? getVarForTarget(toVars, track, target, targetIndex)
                : getDescriptorDefault(
                    descriptor,
                    target,
                    track.name,
                    getVarForTarget(fromVars, track, target, targetIndex)
                );
            var fromVal = hasVar(fromVars, track)
                ? getVarForTarget(fromVars, track, target, targetIndex)
                : getDescriptorDefault(descriptor, target, track.name, toVal);

            if (track.prepared == null && typeof descriptor.prepare === 'function') {
                track.prepared = descriptor.prepare({
                    target: target,
                    from: fromVal,
                    to: toVal,
                    timing: timing,
                    vars: toVars,
                    core: runtime.core,
                });
            }

            var value;
            if (track.keyframes) {
                value = evaluateKeyframes(progress, track.keyframes);
            } else if (tweenContext.sampleOverrides[track.name]) {
                value = tweenContext.sampleOverrides[track.name]({
                    target: target,
                    from: fromVal,
                    to: toVal,
                    progress: progress,
                    handle: handle,
                    timing: timing,
                    core: runtime.core,
                });
            } else if (typeof descriptor.sample === 'function') {
                value = descriptor.sample({
                    target: target,
                    from: fromVal,
                    to: toVal,
                    progress: progress,
                    handle: handle,
                    timing: timing,
                    vars: toVars,
                    prepared: track.prepared,
                    core: runtime.core,
                });
            } else if (descriptor.interpolate === 'color') {
                value = __animate_color(handle, track.name, String(fromVal), String(toVal));
            } else if (descriptor.interpolate === 'number' || descriptor.interpolate == null) {
                value = __animate_value(handle, track.name, Number(fromVal), Number(toVal));
            } else if (typeof descriptor.interpolate === 'function') {
                value = descriptor.interpolate(fromVal, toVal, progress, {
                    target: target,
                    handle: handle,
                    timing: timing,
                    core: runtime.core,
                });
            } else {
                throw new Error(
                    'unsupported interpolation `' + descriptor.interpolate + '` for property `' + track.inputName + '`'
                );
            }

            result.values[track.inputName] = value;
            result[track.inputName] = value;
            if (track.name !== track.inputName) {
                result.values[track.name] = value;
                result[track.name] = value;
            }

            descriptor.apply(target, value, {
                inputName: track.inputName,
                name: track.name,
                timing: timing,
                vars: toVars,
                core: runtime.core,
            });
        }

        return result;
    }

    function applyTween(targets, fromVars, toVars, timing) {
        var list = normalizeTargets(targets);
        var stagger = timing && timing.stagger !== undefined ? timing.stagger : 0;
        var delays = resolveStaggerDelays(stagger, list.length);

        // Auto-fit stagger into remaining scene time. All timing values are seconds.
        if (delays && !(timing && timing.__skipSceneFit)) {
            var sceneDuration = Number(ctx.sceneDuration || ctx.duration || ctx.totalDuration || 0);
            var baseDelay = Number((timing && timing.delay) || 0);
            var duration = Number((timing && timing.duration) || 0);
            var available = sceneDuration - baseDelay - duration;
            if (sceneDuration > 0 && available >= 0) {
                var maxDelay = 0;
                for (var d = 0; d < delays.length; d++) {
                    if (delays[d] > maxDelay) maxDelay = delays[d];
                }
                if (maxDelay > available && maxDelay > 0) {
                    var scale = available / maxDelay;
                    for (var d = 0; d < delays.length; d++) {
                        delays[d] *= scale;
                    }
                }
            }
        }

        var results = [];
        for (var i = 0; i < list.length; i++) {
            var localTiming = copyOwn(timing, {
                delay: Number((timing && timing.delay) || 0) + (delays ? delays[i] : 0),
            });
            var tween = createTween(createTarget(list[i]), fromVars, toVars, localTiming, i);
            results.push(tween);
        }
        return list.length === 1 ? results[0] : results;
    }

    runtime.core = {
        interpolate: {
            number: function(from, to, progress) {
                return Number(from) + (Number(to) - Number(from)) * Number(progress);
            },
        },
        resolveEasingTag: resolveEasingTag,
        normalizeSelector: normalizeSelector,
        normalizeTargets: normalizeTargets,
        createTarget: createTarget,
        splitTiming: splitTiming,
        getDescriptorDefault: getDescriptorDefault,
        getVar: getVar,
        collectTracks: collectTracks,
        createTween: createTween,
        applyTween: applyTween,
        resolveStaggerDelays: resolveStaggerDelays,
    };
})();
