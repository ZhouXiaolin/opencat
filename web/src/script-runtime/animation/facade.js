(function() {
    var runtime = globalThis.__opencatAnimation;
    var core = runtime.core;
    var copyOwn = runtime.copyOwn;
    var hasOwn = runtime.hasOwn;
    var pendingTimelines = [];
    var flushingTimelines = false;

    function flushPendingTimelines() {
        if (flushingTimelines || pendingTimelines.length === 0) {
            return;
        }
        flushingTimelines = true;
        var timelines = pendingTimelines;
        pendingTimelines = [];
        try {
            for (var i = 0; i < timelines.length; i++) {
                timelines[i].flush();
            }
        } finally {
            flushingTimelines = false;
        }
    }

    function parsePosition(pos, state) {
        if (pos == null) return state.cursor;
        if (typeof pos === 'number') return pos;
        if (typeof pos === 'string') {
            var prevStart = state.previousStart == null ? state.cursor : state.previousStart;
            var prevEnd = state.previousEnd == null ? state.cursor : state.previousEnd;

            if (pos === '<') return prevStart;
            if (pos === '>') return prevEnd;

            var relativeToPrevious = pos.match(/^([<>])([+-]=)(-?\d+(?:\.\d+)?)$/);
            if (relativeToPrevious) {
                var base = relativeToPrevious[1] === '<' ? prevStart : prevEnd;
                var amount = Number(relativeToPrevious[3]);
                return relativeToPrevious[2] === '+=' ? base + amount : base - amount;
            }

            var shorthandRelativeToPrevious = pos.match(/^([<>])([+-]?\d+(?:\.\d+)?)$/);
            if (shorthandRelativeToPrevious) {
                var shorthandBase = shorthandRelativeToPrevious[1] === '<' ? prevStart : prevEnd;
                return shorthandBase + Number(shorthandRelativeToPrevious[2]);
            }

            if (pos.indexOf('+=') === 0) return state.cursor + Number(pos.slice(2));
            if (pos.indexOf('-=') === 0) return state.cursor - Number(pos.slice(2));

            var labelRelative = pos.match(/^(.+?)([+-]=)(-?\d+(?:\.\d+)?)$/);
            if (labelRelative && hasOwn(state.labels, labelRelative[1])) {
                var labelBase = state.labels[labelRelative[1]];
                var labelAmount = Number(labelRelative[3]);
                return labelRelative[2] === '+=' ? labelBase + labelAmount : labelBase - labelAmount;
            }

            if (hasOwn(state.labels, pos)) return state.labels[pos];
            var n = Number(pos);
            if (!isNaN(n)) return n;
        }
        throw new Error('unsupported timeline position `' + pos + '`');
    }

    function applySet(targets, vars) {
        var list = core.normalizeTargets(targets);
        vars = vars || {};
        var trackSet = core.collectTracks({}, vars, {});
        for (var i = 0; i < list.length; i++) {
            var target = core.createTarget(list[i]);
            for (var t = 0; t < trackSet.tracks.length; t++) {
                var track = trackSet.tracks[t];
                var value = core.getVar(vars, track);
                track.descriptor.apply(target, value, {
                    inputName: track.inputName,
                    name: track.name,
                    timing: {},
                    vars: vars,
                    core: core,
                });
            }
        }
        return list;
    }

    ctx.__flushTimelines = flushPendingTimelines;

    ctx.set = function(targets, vars) {
        flushPendingTimelines();
        return applySet(targets, vars);
    };

    ctx.to = function(targets, vars) {
        flushPendingTimelines();
        return core.applyTween(targets, {}, vars || {}, core.splitTiming(vars));
    };

    ctx.from = function(targets, vars) {
        flushPendingTimelines();
        vars = vars || {};
        var toVars = {};
        var tracks = core.collectTracks(vars, {}, {}).tracks;
        for (var i = 0; i < tracks.length; i++) {
            var track = tracks[i];
            toVars[track.inputName] = core.getDescriptorDefault(
                track.descriptor,
                null,
                track.name,
                core.getVar(vars, track)
            );
        }
        return core.applyTween(targets, vars, toVars, core.splitTiming(vars));
    };

    ctx.fromTo = function(targets, fromVars, toVars) {
        flushPendingTimelines();
        return core.applyTween(targets, fromVars || {}, toVars || {}, core.splitTiming(toVars));
    };

    ctx.timeline = function(opts) {
        opts = opts || {};
        var baseDelay = Number(opts.delay || 0);
        var defaults = opts.defaults || {};
        var state = {
            labels: {},
            cursor: 0,
            previousStart: null,
            previousEnd: null,
            writes: [],
            items: [],
        };
        var scheduled = false;

        function scheduleFlush() {
            if (!scheduled) {
                scheduled = true;
                pendingTimelines.push(api);
            }
        }

        function recordChild(start, duration) {
            state.previousStart = start;
            state.previousEnd = start + duration;
        }

        function hasWrite(target, prop) {
            for (var i = 0; i < state.writes.length; i++) {
                var write = state.writes[i];
                if (write.target === target && write.prop === prop) {
                    return true;
                }
            }
            return false;
        }

        function markWrite(target, prop) {
            if (!hasWrite(target, prop)) {
                state.writes.push({ target: target, prop: prop });
            }
        }

        function collectTweenTracks(fromVars, toVars, timing) {
            return core.collectTracks(fromVars || {}, toVars || {}, timing || {}).tracks;
        }

        function markTweenWrites(targets, fromVars, toVars, timing) {
            var list = core.normalizeTargets(targets);
            var tracks = collectTweenTracks(fromVars, toVars, timing);
            for (var i = 0; i < list.length; i++) {
                for (var t = 0; t < tracks.length; t++) {
                    markWrite(list[i], tracks[t].name);
                }
            }
        }

        function resolveImmediateValue(value, target, targetIndex) {
            if (typeof value === 'function') {
                return value(targetIndex || 0, target ? target.raw : undefined);
            }
            return value;
        }

        function applyInitialValues(targets, vars) {
            var list = core.normalizeTargets(targets);
            var tracks = collectTweenTracks(vars, {}, {});
            for (var i = 0; i < list.length; i++) {
                var rawTarget = list[i];
                var target = core.createTarget(rawTarget);
                for (var t = 0; t < tracks.length; t++) {
                    var track = tracks[t];
                    if (hasWrite(rawTarget, track.name)) {
                        continue;
                    }
                    var value = resolveImmediateValue(core.getVar(vars, track), target, i);
                    track.descriptor.apply(target, value, {
                        inputName: track.inputName,
                        name: track.name,
                        timing: {},
                        vars: vars,
                        core: core,
                    });
                    markWrite(rawTarget, track.name);
                }
            }
        }

        function targetCount(targets) {
            return core.normalizeTargets(targets).length;
        }

        function tweenSpan(targets, duration, timing) {
            var stagger = timing && timing.stagger !== undefined ? Number(timing.stagger) : 0;
            return Number(duration || 0) + Math.max(0, targetCount(targets) - 1) * Math.max(0, stagger);
        }

        function scaledTiming(timing, scale) {
            var out = copyOwn({}, timing);
            if (out.delay !== undefined) {
                out.delay = Number(out.delay) * scale;
            }
            if (out.duration !== undefined) {
                out.duration = Number(out.duration) * scale;
            }
            if (out.repeatDelay !== undefined) {
                out.repeatDelay = Number(out.repeatDelay) * scale;
            }
            if (out.stagger !== undefined) {
                out.stagger = Number(out.stagger) * scale;
            }
            out.__skipSceneFit = true;
            return out;
        }

        function addTween(kind, targets, a, b, pos) {
            var fromVars;
            var toVars;
            if (kind === 'fromTo') {
                fromVars = a || {};
                toVars = b || {};
            } else if (kind === 'from') {
                fromVars = a || {};
                toVars = {};
                var tracks = core.collectTracks(fromVars, {}, {}).tracks;
                for (var fi = 0; fi < tracks.length; fi++) {
                    var track = tracks[fi];
                    toVars[track.inputName] = core.getDescriptorDefault(
                        track.descriptor,
                        null,
                        track.name,
                        core.getVar(fromVars, track)
                    );
                }
            } else {
                fromVars = {};
                toVars = a || {};
            }

            var varsForTiming = kind === 'fromTo' ? toVars : a;
            var hasExplicitPosition = pos != null || (varsForTiming && varsForTiming.at != null);
            var positionValue = hasExplicitPosition ? (pos != null ? pos : varsForTiming.at) : null;
            var start = parsePosition(positionValue, state);
            var mergedTiming = core.splitTiming(copyOwn(defaults, varsForTiming), baseDelay + start);
            var duration = mergedTiming.duration !== undefined ? Number(mergedTiming.duration) : 0;
            var span = tweenSpan(targets, duration, mergedTiming);
            state.cursor = Math.max(state.cursor, start + span);
            recordChild(start, span);
            state.items.push({
                kind: kind,
                targets: targets,
                fromVars: fromVars,
                toVars: toVars,
                varsForTiming: varsForTiming,
                start: start,
            });
            scheduleFlush();
            return null;
        }

        var api = {
            flush: function() {
                var sceneFrames = Number(ctx.sceneFrames || ctx.totalFrames || 0);
                var totalEnd = baseDelay + state.cursor;
                var scale = sceneFrames > 0 && totalEnd > sceneFrames ? sceneFrames / totalEnd : 1;

                for (var i = 0; i < state.items.length; i++) {
                    var item = state.items[i];
                    if (item.kind === 'set') {
                        var setStart = (baseDelay + item.start) * scale;
                        if (ctx.currentFrame >= setStart) {
                            applySet(item.targets, item.vars || {});
                            var setTracks = collectTweenTracks({}, item.vars || {}, {});
                            var setTargets = core.normalizeTargets(item.targets);
                            for (var si = 0; si < setTargets.length; si++) {
                                for (var st = 0; st < setTracks.length; st++) {
                                    markWrite(setTargets[si], setTracks[st].name);
                                }
                            }
                        }
                        continue;
                    }

                    var baseTiming = core.splitTiming(
                        copyOwn(defaults, item.varsForTiming),
                        baseDelay + item.start
                    );
                    var timing = scaledTiming(baseTiming, scale);
                    if (ctx.currentFrame < Number(timing.delay || 0)) {
                        if (item.kind === 'from' || item.kind === 'fromTo') {
                            applyInitialValues(item.targets, item.fromVars);
                        }
                        continue;
                    }
                    var mergedToVars = copyOwn(defaults, item.toVars);
                    var result = core.applyTween(item.targets, item.fromVars, mergedToVars, timing);
                    markTweenWrites(item.targets, item.fromVars, mergedToVars, timing);
                }
                state.items = [];
            },
            set: function(targets, vars, pos) {
                var start = parsePosition(pos != null ? pos : vars && vars.at, state);
                state.cursor = Math.max(state.cursor, start);
                recordChild(start, 0);
                state.items.push({
                    kind: 'set',
                    targets: targets,
                    vars: vars || {},
                    start: start,
                });
                scheduleFlush();
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
                state.labels[String(name)] = parsePosition(pos, state);
                return api;
            },
        };
        return api;
    };
})();
