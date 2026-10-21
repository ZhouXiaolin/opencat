var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

// scripts/gsap-entry.mjs
var gsap_entry_exports = {};
__export(gsap_entry_exports, {
  Back: () => Back,
  Bounce: () => Bounce,
  CSSPlugin: () => CSSPlugin,
  CSSRulePlugin: () => CSSRulePlugin,
  Circ: () => Circ,
  Cubic: () => Cubic,
  CustomBounce: () => CustomBounce,
  CustomEase: () => CustomEase,
  CustomWiggle: () => CustomWiggle,
  Draggable: () => Draggable,
  DrawSVGPlugin: () => DrawSVGPlugin,
  EaselPlugin: () => EaselPlugin,
  Elastic: () => Elastic,
  Expo: () => Expo,
  ExpoScaleEase: () => ExpoScaleEase,
  Flip: () => Flip,
  GSDevTools: () => GSDevTools,
  InertiaPlugin: () => InertiaPlugin2,
  Linear: () => Linear,
  MorphSVGPlugin: () => MorphSVGPlugin,
  MotionPathHelper: () => MotionPathHelper,
  MotionPathPlugin: () => MotionPathPlugin,
  Observer: () => Observer,
  Physics2DPlugin: () => Physics2DPlugin,
  PhysicsPropsPlugin: () => PhysicsPropsPlugin,
  PixiPlugin: () => PixiPlugin,
  Power0: () => Power0,
  Power1: () => Power1,
  Power2: () => Power2,
  Power3: () => Power3,
  Power4: () => Power4,
  Quad: () => Quad,
  Quart: () => Quart,
  Quint: () => Quint,
  RoughEase: () => RoughEase,
  ScrambleTextPlugin: () => ScrambleTextPlugin,
  ScrollSmoother: () => ScrollSmoother,
  ScrollToPlugin: () => ScrollToPlugin,
  ScrollTrigger: () => ScrollTrigger2,
  Sine: () => Sine,
  SlowMo: () => SlowMo,
  SplitText: () => SplitText,
  SteppedEase: () => SteppedEase,
  Strong: () => Strong,
  TextPlugin: () => TextPlugin,
  TimelineLite: () => Timeline,
  TimelineMax: () => Timeline,
  TweenLite: () => Tween,
  default: () => gsapWithCSS,
  gsap: () => gsapWithCSS
});
module.exports = __toCommonJS(gsap_entry_exports);

// examples/figma-launch/node_modules/gsap/gsap-core.js
function _assertThisInitialized(self) {
  if (self === void 0) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }
  return self;
}
function _inheritsLoose(subClass, superClass) {
  subClass.prototype = Object.create(superClass.prototype);
  subClass.prototype.constructor = subClass;
  subClass.__proto__ = superClass;
}
var _config = {
  autoSleep: 120,
  force3D: "auto",
  nullTargetWarn: 1,
  units: {
    lineHeight: ""
  }
};
var _defaults = {
  duration: 0.5,
  overwrite: false,
  delay: 0
};
var _suppressOverwrites;
var _reverting;
var _context;
var _bigNum = 1e8;
var _tinyNum = 1 / _bigNum;
var _2PI = Math.PI * 2;
var _HALF_PI = _2PI / 4;
var _gsID = 0;
var _sqrt = Math.sqrt;
var _cos = Math.cos;
var _sin = Math.sin;
var _isString = function _isString2(value) {
  return typeof value === "string";
};
var _isFunction = function _isFunction2(value) {
  return typeof value === "function";
};
var _isNumber = function _isNumber2(value) {
  return typeof value === "number";
};
var _isUndefined = function _isUndefined2(value) {
  return typeof value === "undefined";
};
var _isObject = function _isObject2(value) {
  return typeof value === "object";
};
var _isNotFalse = function _isNotFalse2(value) {
  return value !== false;
};
var _windowExists = function _windowExists2() {
  return typeof window !== "undefined";
};
var _isFuncOrString = function _isFuncOrString2(value) {
  return _isFunction(value) || _isString(value);
};
var _isTypedArray = typeof ArrayBuffer === "function" && ArrayBuffer.isView || function() {
};
var _isArray = Array.isArray;
var _randomExp = /random\([^)]+\)/g;
var _commaDelimExp = /,\s*/g;
var _strictNumExp = /(?:-?\.?\d|\.)+/gi;
var _numExp = /[-+=.]*\d+[.e\-+]*\d*[e\-+]*\d*/g;
var _numWithUnitExp = /[-+=.]*\d+[.e-]*\d*[a-z%]*/g;
var _complexStringNumExp = /[-+=.]*\d+\.?\d*(?:e-|e\+)?\d*/gi;
var _relExp = /[+-]=-?[.\d]+/;
var _delimitedValueExp = /[^,'"\[\]\s]+/gi;
var _unitExp = /^[+\-=e\s\d]*\d+[.\d]*([a-z]*|%)\s*$/i;
var _globalTimeline;
var _win;
var _coreInitted;
var _doc;
var _globals = {};
var _installScope = {};
var _coreReady;
var _install = function _install2(scope) {
  return (_installScope = _merge(scope, _globals)) && gsap;
};
var _missingPlugin = function _missingPlugin2(property, value) {
  return console.warn("Invalid property", property, "set to", value, "Missing plugin? gsap.registerPlugin()");
};
var _warn = function _warn2(message, suppress) {
  return !suppress && console.warn(message);
};
var _addGlobal = function _addGlobal2(name, obj) {
  return name && (_globals[name] = obj) && _installScope && (_installScope[name] = obj) || _globals;
};
var _emptyFunc = function _emptyFunc2() {
  return 0;
};
var _startAtRevertConfig = {
  suppressEvents: true,
  isStart: true,
  kill: false
};
var _revertConfigNoKill = {
  suppressEvents: true,
  kill: false
};
var _revertConfig = {
  suppressEvents: true
};
var _reservedProps = {};
var _lazyTweens = [];
var _lazyLookup = {};
var _lastRenderedFrame;
var _plugins = {};
var _effects = {};
var _nextGCFrame = 30;
var _harnessPlugins = [];
var _callbackNames = "";
var _harness = function _harness2(targets) {
  var target = targets[0], harnessPlugin, i2;
  _isObject(target) || _isFunction(target) || (targets = [targets]);
  if (!(harnessPlugin = (target._gsap || {}).harness)) {
    i2 = _harnessPlugins.length;
    while (i2-- && !_harnessPlugins[i2].targetTest(target)) {
    }
    harnessPlugin = _harnessPlugins[i2];
  }
  i2 = targets.length;
  while (i2--) {
    targets[i2] && (targets[i2]._gsap || (targets[i2]._gsap = new GSCache(targets[i2], harnessPlugin))) || targets.splice(i2, 1);
  }
  return targets;
};
var _getCache = function _getCache2(target) {
  return target._gsap || _harness(toArray(target))[0]._gsap;
};
var _getProperty = function _getProperty2(target, property, v) {
  return (v = target[property]) && _isFunction(v) ? target[property]() : _isUndefined(v) && target.getAttribute && target.getAttribute(property) || v;
};
var _forEachName = function _forEachName2(names, func) {
  return (names = names.split(",")).forEach(func) || names;
};
var _round = function _round2(value) {
  return Math.round(value * 1e5) / 1e5 || 0;
};
var _roundPrecise = function _roundPrecise2(value) {
  return Math.round(value * 1e7) / 1e7 || 0;
};
var _parseRelative = function _parseRelative2(start, value) {
  var operator = value.charAt(0), end = parseFloat(value.substr(2));
  start = parseFloat(start);
  return operator === "+" ? start + end : operator === "-" ? start - end : operator === "*" ? start * end : start / end;
};
var _arrayContainsAny = function _arrayContainsAny2(toSearch, toFind) {
  var l = toFind.length, i2 = 0;
  for (; toSearch.indexOf(toFind[i2]) < 0 && ++i2 < l; ) {
  }
  return i2 < l;
};
var _lazyRender = function _lazyRender2() {
  var l = _lazyTweens.length, a = _lazyTweens.slice(0), i2, tween;
  _lazyLookup = {};
  _lazyTweens.length = 0;
  for (i2 = 0; i2 < l; i2++) {
    tween = a[i2];
    tween && tween._lazy && (tween.render(tween._lazy[0], tween._lazy[1], true)._lazy = 0);
  }
};
var _isRevertWorthy = function _isRevertWorthy2(animation) {
  return !!(animation._initted || animation._startAt || animation.add);
};
var _lazySafeRender = function _lazySafeRender2(animation, time, suppressEvents, force) {
  _lazyTweens.length && !_reverting && _lazyRender();
  animation.render(time, suppressEvents, force || !!(_reverting && time < 0 && _isRevertWorthy(animation)));
  _lazyTweens.length && !_reverting && _lazyRender();
};
var _numericIfPossible = function _numericIfPossible2(value) {
  var n = parseFloat(value);
  return (n || n === 0) && (value + "").match(_delimitedValueExp).length < 2 ? n : _isString(value) ? value.trim() : value;
};
var _passThrough = function _passThrough2(p2) {
  return p2;
};
var _setDefaults = function _setDefaults2(obj, defaults2) {
  for (var p2 in defaults2) {
    p2 in obj || (obj[p2] = defaults2[p2]);
  }
  return obj;
};
var _setKeyframeDefaults = function _setKeyframeDefaults2(excludeDuration) {
  return function(obj, defaults2) {
    for (var p2 in defaults2) {
      p2 in obj || p2 === "duration" && excludeDuration || p2 === "ease" || (obj[p2] = defaults2[p2]);
    }
  };
};
var _merge = function _merge2(base, toMerge) {
  for (var p2 in toMerge) {
    base[p2] = toMerge[p2];
  }
  return base;
};
var _mergeDeep = function _mergeDeep2(base, toMerge) {
  for (var p2 in toMerge) {
    p2 !== "__proto__" && p2 !== "constructor" && p2 !== "prototype" && (base[p2] = _isObject(toMerge[p2]) ? _mergeDeep2(base[p2] || (base[p2] = {}), toMerge[p2]) : toMerge[p2]);
  }
  return base;
};
var _copyExcluding = function _copyExcluding2(obj, excluding) {
  var copy = {}, p2;
  for (p2 in obj) {
    p2 in excluding || (copy[p2] = obj[p2]);
  }
  return copy;
};
var _inheritDefaults = function _inheritDefaults2(vars) {
  var parent = vars.parent || _globalTimeline, func = vars.keyframes ? _setKeyframeDefaults(_isArray(vars.keyframes)) : _setDefaults;
  if (_isNotFalse(vars.inherit)) {
    while (parent) {
      func(vars, parent.vars.defaults);
      parent = parent.parent || parent._dp;
    }
  }
  return vars;
};
var _arraysMatch = function _arraysMatch2(a1, a2) {
  var i2 = a1.length, match = i2 === a2.length;
  while (match && i2-- && a1[i2] === a2[i2]) {
  }
  return i2 < 0;
};
var _addLinkedListItem = function _addLinkedListItem2(parent, child, firstProp, lastProp, sortBy) {
  if (firstProp === void 0) {
    firstProp = "_first";
  }
  if (lastProp === void 0) {
    lastProp = "_last";
  }
  var prev = parent[lastProp], t;
  if (sortBy) {
    t = child[sortBy];
    while (prev && prev[sortBy] > t) {
      prev = prev._prev;
    }
  }
  if (prev) {
    child._next = prev._next;
    prev._next = child;
  } else {
    child._next = parent[firstProp];
    parent[firstProp] = child;
  }
  if (child._next) {
    child._next._prev = child;
  } else {
    parent[lastProp] = child;
  }
  child._prev = prev;
  child.parent = child._dp = parent;
  return child;
};
var _removeLinkedListItem = function _removeLinkedListItem2(parent, child, firstProp, lastProp) {
  if (firstProp === void 0) {
    firstProp = "_first";
  }
  if (lastProp === void 0) {
    lastProp = "_last";
  }
  var prev = child._prev, next = child._next;
  if (prev) {
    prev._next = next;
  } else if (parent[firstProp] === child) {
    parent[firstProp] = next;
  }
  if (next) {
    next._prev = prev;
  } else if (parent[lastProp] === child) {
    parent[lastProp] = prev;
  }
  child._next = child._prev = child.parent = null;
};
var _removeFromParent = function _removeFromParent2(child, onlyIfParentHasAutoRemove) {
  child.parent && (!onlyIfParentHasAutoRemove || child.parent.autoRemoveChildren) && child.parent.remove && child.parent.remove(child);
  child._act = 0;
};
var _uncache = function _uncache2(animation, child) {
  if (animation && (!child || child._end > animation._dur || child._start < 0)) {
    var a = animation;
    while (a) {
      a._dirty = 1;
      a = a.parent;
    }
  }
  return animation;
};
var _recacheAncestors = function _recacheAncestors2(animation) {
  var parent = animation.parent;
  while (parent && parent.parent) {
    parent._dirty = 1;
    parent.totalDuration();
    parent = parent.parent;
  }
  return animation;
};
var _rewindStartAt = function _rewindStartAt2(tween, totalTime, suppressEvents, force) {
  return tween._startAt && (_reverting ? tween._startAt.revert(_revertConfigNoKill) : tween.vars.immediateRender && !tween.vars.autoRevert || tween._startAt.render(totalTime, true, force));
};
var _hasNoPausedAncestors = function _hasNoPausedAncestors2(animation) {
  return !animation || animation._ts && _hasNoPausedAncestors2(animation.parent);
};
var _elapsedCycleDuration = function _elapsedCycleDuration2(animation) {
  return animation._repeat ? _animationCycle(animation._tTime, animation = animation.duration() + animation._rDelay) * animation : 0;
};
var _animationCycle = function _animationCycle2(tTime, cycleDuration) {
  var whole = Math.floor(tTime = _roundPrecise(tTime / cycleDuration));
  return tTime && whole === tTime ? whole - 1 : whole;
};
var _parentToChildTotalTime = function _parentToChildTotalTime2(parentTime, child) {
  return (parentTime - child._start) * child._ts + (child._ts >= 0 ? 0 : child._dirty ? child.totalDuration() : child._tDur);
};
var _setEnd = function _setEnd2(animation) {
  return animation._end = _roundPrecise(animation._start + (animation._tDur / Math.abs(animation._ts || animation._rts || _tinyNum) || 0));
};
var _alignPlayhead = function _alignPlayhead2(animation, totalTime) {
  var parent = animation._dp;
  if (parent && parent.smoothChildTiming && animation._ts) {
    animation._start = _roundPrecise(parent._time - (animation._ts > 0 ? totalTime / animation._ts : ((animation._dirty ? animation.totalDuration() : animation._tDur) - totalTime) / -animation._ts));
    _setEnd(animation);
    parent._dirty || _uncache(parent, animation);
  }
  return animation;
};
var _postAddChecks = function _postAddChecks2(timeline2, child) {
  var t;
  if (child._time || !child._dur && child._initted || child._start < timeline2._time && (child._dur || !child.add)) {
    t = _parentToChildTotalTime(timeline2.rawTime(), child);
    if (!child._dur || _clamp(0, child.totalDuration(), t) - child._tTime > _tinyNum) {
      child.render(t, true);
    }
  }
  if (_uncache(timeline2, child)._dp && timeline2._initted && timeline2._time >= timeline2._dur && timeline2._ts) {
    if (timeline2._dur < timeline2.duration()) {
      t = timeline2;
      while (t._dp) {
        t.rawTime() >= 0 && t.totalTime(t._tTime);
        t = t._dp;
      }
    }
    timeline2._zTime = -_tinyNum;
  }
};
var _addToTimeline = function _addToTimeline2(timeline2, child, position, skipChecks) {
  child.parent && _removeFromParent(child);
  child._start = _roundPrecise((_isNumber(position) ? position : position || timeline2 !== _globalTimeline ? _parsePosition(timeline2, position, child) : timeline2._time) + child._delay);
  child._end = _roundPrecise(child._start + (child.totalDuration() / Math.abs(child.timeScale()) || 0));
  _addLinkedListItem(timeline2, child, "_first", "_last", timeline2._sort ? "_start" : 0);
  _isFromOrFromStart(child) || (timeline2._recent = child);
  skipChecks || _postAddChecks(timeline2, child);
  timeline2._ts < 0 && _alignPlayhead(timeline2, timeline2._tTime);
  return timeline2;
};
var _scrollTrigger = function _scrollTrigger2(animation, trigger) {
  return (_globals.ScrollTrigger || _missingPlugin("scrollTrigger", trigger)) && _globals.ScrollTrigger.create(trigger, animation);
};
var _attemptInitTween = function _attemptInitTween2(tween, time, force, suppressEvents, tTime) {
  _initTween(tween, time, tTime);
  if (!tween._initted) {
    return 1;
  }
  if (!force && tween._pt && !_reverting && (tween._dur && tween.vars.lazy !== false || !tween._dur && tween.vars.lazy) && _lastRenderedFrame !== _ticker.frame) {
    _lazyTweens.push(tween);
    tween._lazy = [tTime, suppressEvents];
    return 1;
  }
};
var _parentPlayheadIsBeforeStart = function _parentPlayheadIsBeforeStart2(_ref) {
  var parent = _ref.parent;
  return parent && parent._ts && parent._initted && !parent._lock && (parent.rawTime() < 0 || _parentPlayheadIsBeforeStart2(parent));
};
var _isFromOrFromStart = function _isFromOrFromStart2(_ref2) {
  var data = _ref2.data;
  return data === "isFromStart" || data === "isStart";
};
var _renderZeroDurationTween = function _renderZeroDurationTween2(tween, totalTime, suppressEvents, force) {
  var prevRatio = tween.ratio, ratio = totalTime < 0 || !totalTime && (!tween._start && _parentPlayheadIsBeforeStart(tween) && !(!tween._initted && _isFromOrFromStart(tween)) || (tween._ts < 0 || tween._dp._ts < 0) && !_isFromOrFromStart(tween)) ? 0 : 1, repeatDelay = tween._rDelay, tTime = 0, pt, iteration, prevIteration;
  if (repeatDelay && tween._repeat) {
    tTime = _clamp(0, tween._tDur, totalTime);
    iteration = _animationCycle(tTime, repeatDelay);
    tween._yoyo && iteration & 1 && (ratio = 1 - ratio);
    if (iteration !== _animationCycle(tween._tTime, repeatDelay)) {
      prevRatio = 1 - ratio;
      tween.vars.repeatRefresh && tween._initted && tween.invalidate();
    }
  }
  if (ratio !== prevRatio || _reverting || force || tween._zTime === _tinyNum || !totalTime && tween._zTime) {
    if (!tween._initted && _attemptInitTween(tween, totalTime, force, suppressEvents, tTime)) {
      return;
    }
    prevIteration = tween._zTime;
    tween._zTime = totalTime || (suppressEvents ? _tinyNum : 0);
    suppressEvents || (suppressEvents = totalTime && !prevIteration);
    tween.ratio = ratio;
    tween._from && (ratio = 1 - ratio);
    tween._time = 0;
    tween._tTime = tTime;
    pt = tween._pt;
    while (pt) {
      pt.r(ratio, pt.d);
      pt = pt._next;
    }
    totalTime < 0 && _rewindStartAt(tween, totalTime, suppressEvents, true);
    tween._onUpdate && !suppressEvents && _callback(tween, "onUpdate");
    tTime && tween._repeat && !suppressEvents && tween.parent && _callback(tween, "onRepeat");
    if ((totalTime >= tween._tDur || totalTime < 0) && tween.ratio === ratio) {
      ratio && _removeFromParent(tween, 1);
      if (!suppressEvents && !_reverting) {
        _callback(tween, ratio ? "onComplete" : "onReverseComplete", true);
        tween._prom && tween._prom();
      }
    }
  } else if (!tween._zTime) {
    tween._zTime = totalTime;
  }
};
var _findNextPauseTween = function _findNextPauseTween2(animation, prevTime, time) {
  var child;
  if (time > prevTime) {
    child = animation._first;
    while (child && child._start <= time) {
      if (child.data === "isPause" && child._start > prevTime) {
        return child;
      }
      child = child._next;
    }
  } else {
    child = animation._last;
    while (child && child._start >= time) {
      if (child.data === "isPause" && child._start < prevTime) {
        return child;
      }
      child = child._prev;
    }
  }
};
var _setDuration = function _setDuration2(animation, duration, skipUncache, leavePlayhead) {
  var repeat = animation._repeat, dur = _roundPrecise(duration) || 0, totalProgress = animation._tTime / animation._tDur;
  totalProgress && !leavePlayhead && (animation._time *= dur / animation._dur);
  animation._dur = dur;
  animation._tDur = !repeat ? dur : repeat < 0 ? 1e10 : _roundPrecise(dur * (repeat + 1) + animation._rDelay * repeat);
  totalProgress > 0 && !leavePlayhead && _alignPlayhead(animation, animation._tTime = animation._tDur * totalProgress);
  animation.parent && _setEnd(animation);
  skipUncache || _uncache(animation.parent, animation);
  return animation;
};
var _onUpdateTotalDuration = function _onUpdateTotalDuration2(animation) {
  return animation instanceof Timeline ? _uncache(animation) : _setDuration(animation, animation._dur);
};
var _zeroPosition = {
  _start: 0,
  endTime: _emptyFunc,
  totalDuration: _emptyFunc
};
var _parsePosition = function _parsePosition2(animation, position, percentAnimation) {
  var labels = animation.labels, recent = animation._recent || _zeroPosition, clippedDuration = animation.duration() >= _bigNum ? recent.endTime(false) : animation._dur, i2, offset, isPercent;
  if (_isString(position) && (isNaN(position) || position in labels)) {
    offset = position.charAt(0);
    isPercent = position.substr(-1) === "%";
    i2 = position.indexOf("=");
    if (offset === "<" || offset === ">") {
      i2 >= 0 && (position = position.replace(/=/, ""));
      return (offset === "<" ? recent._start : recent.endTime(recent._repeat >= 0)) + (parseFloat(position.substr(1)) || 0) * (isPercent ? (i2 < 0 ? recent : percentAnimation).totalDuration() / 100 : 1);
    }
    if (i2 < 0) {
      position in labels || (labels[position] = clippedDuration);
      return labels[position];
    }
    offset = parseFloat(position.charAt(i2 - 1) + position.substr(i2 + 1));
    if (isPercent && percentAnimation) {
      offset = offset / 100 * (_isArray(percentAnimation) ? percentAnimation[0] : percentAnimation).totalDuration();
    }
    return i2 > 1 ? _parsePosition2(animation, position.substr(0, i2 - 1), percentAnimation) + offset : clippedDuration + offset;
  }
  return position == null ? clippedDuration : +position;
};
var _createTweenType = function _createTweenType2(type, params, timeline2) {
  var isLegacy = _isNumber(params[1]), varsIndex = (isLegacy ? 2 : 1) + (type < 2 ? 0 : 1), vars = params[varsIndex], irVars, parent;
  isLegacy && (vars.duration = params[1]);
  vars.parent = timeline2;
  if (type) {
    irVars = vars;
    parent = timeline2;
    while (parent && !("immediateRender" in irVars)) {
      irVars = parent.vars.defaults || {};
      parent = _isNotFalse(parent.vars.inherit) && parent.parent;
    }
    vars.immediateRender = _isNotFalse(irVars.immediateRender);
    type < 2 ? vars.runBackwards = 1 : vars.startAt = params[varsIndex - 1];
  }
  return new Tween(params[0], vars, params[varsIndex + 1]);
};
var _conditionalReturn = function _conditionalReturn2(value, func) {
  return value || value === 0 ? func(value) : func;
};
var _clamp = function _clamp2(min, max, value) {
  return value < min ? min : value > max ? max : value;
};
var getUnit = function getUnit2(value, v) {
  return !_isString(value) || !(v = _unitExp.exec(value)) ? "" : v[1];
};
var clamp = function clamp2(min, max, value) {
  return _conditionalReturn(value, function(v) {
    return _clamp(min, max, v);
  });
};
var _slice = [].slice;
var _isArrayLike = function _isArrayLike2(value, nonEmpty) {
  return value && _isObject(value) && "length" in value && (!nonEmpty && !value.length || value.length - 1 in value && _isObject(value[0])) && !value.nodeType && value !== _win;
};
var _flatten = function _flatten2(ar, leaveStrings, accumulator) {
  if (accumulator === void 0) {
    accumulator = [];
  }
  return ar.forEach(function(value) {
    var _accumulator;
    return _isString(value) && !leaveStrings || _isArrayLike(value, 1) ? (_accumulator = accumulator).push.apply(_accumulator, toArray(value)) : accumulator.push(value);
  }) || accumulator;
};
var toArray = function toArray2(value, scope, leaveStrings) {
  return _context && !scope && _context.selector ? _context.selector(value) : _isString(value) && !leaveStrings && (_coreInitted || !_wake()) ? _slice.call((scope || _doc).querySelectorAll(value), 0) : _isArray(value) ? _flatten(value, leaveStrings) : _isArrayLike(value) ? _slice.call(value, 0) : value ? [value] : [];
};
var selector = function selector2(value) {
  value = toArray(value)[0] || _warn("Invalid scope") || {};
  return function(v) {
    var el = value.current || value.nativeElement || value;
    return toArray(v, el.querySelectorAll ? el : el === value ? _warn("Invalid scope") || _doc.createElement("div") : value);
  };
};
var shuffle = function shuffle2(a) {
  return a.sort(function() {
    return 0.5 - Math.random();
  });
};
var distribute = function distribute2(v) {
  if (_isFunction(v)) {
    return v;
  }
  var vars = _isObject(v) ? v : {
    each: v
  }, ease = _parseEase(vars.ease), from = vars.from || 0, base = parseFloat(vars.base) || 0, cache = {}, isDecimal = from > 0 && from < 1, ratios = isNaN(from) || isDecimal, axis = vars.axis, ratioX = from, ratioY = from;
  if (_isString(from)) {
    ratioX = ratioY = {
      center: 0.5,
      edges: 0.5,
      end: 1
    }[from] || 0;
  } else if (!isDecimal && ratios) {
    ratioX = from[0];
    ratioY = from[1];
  }
  return function(i2, target, a) {
    var l = (a || vars).length, distances = cache[l], originX, originY, x, y, d, j, max, min, wrapAt;
    if (!distances) {
      wrapAt = vars.grid === "auto" ? 0 : (vars.grid || [1, _bigNum])[1];
      if (!wrapAt) {
        max = -_bigNum;
        while (max < (max = a[wrapAt++].getBoundingClientRect().left) && wrapAt < l) {
        }
        wrapAt < l && wrapAt--;
      }
      distances = cache[l] = [];
      originX = ratios ? Math.min(wrapAt, l) * ratioX - 0.5 : from % wrapAt;
      originY = wrapAt === _bigNum ? 0 : ratios ? l * ratioY / wrapAt - 0.5 : from / wrapAt | 0;
      max = 0;
      min = _bigNum;
      for (j = 0; j < l; j++) {
        x = j % wrapAt - originX;
        y = originY - (j / wrapAt | 0);
        distances[j] = d = !axis ? _sqrt(x * x + y * y) : Math.abs(axis === "y" ? y : x);
        d > max && (max = d);
        d < min && (min = d);
      }
      from === "random" && shuffle(distances);
      distances.max = max - min;
      distances.min = min;
      distances.v = l = (parseFloat(vars.amount) || parseFloat(vars.each) * (wrapAt > l ? l - 1 : !axis ? Math.max(wrapAt, l / wrapAt) : axis === "y" ? l / wrapAt : wrapAt) || 0) * (from === "edges" ? -1 : 1);
      distances.b = l < 0 ? base - l : base;
      distances.u = getUnit(vars.amount || vars.each) || 0;
      ease = ease && l < 0 ? _invertEase(ease) : ease;
    }
    l = (distances[i2] - distances.min) / distances.max || 0;
    return _roundPrecise(distances.b + (ease ? ease(l) : l) * distances.v) + distances.u;
  };
};
var _roundModifier = function _roundModifier2(v) {
  var p2 = Math.pow(10, ((v + "").split(".")[1] || "").length);
  return function(raw) {
    var n = _roundPrecise(Math.round(parseFloat(raw) / v) * v * p2);
    return (n - n % 1) / p2 + (_isNumber(raw) ? 0 : getUnit(raw));
  };
};
var snap = function snap2(snapTo, value) {
  var isArray = _isArray(snapTo), radius, is2D;
  if (!isArray && _isObject(snapTo)) {
    radius = isArray = snapTo.radius || _bigNum;
    if (snapTo.values) {
      snapTo = toArray(snapTo.values);
      if (is2D = !_isNumber(snapTo[0])) {
        radius *= radius;
      }
    } else {
      snapTo = _roundModifier(snapTo.increment);
    }
  }
  return _conditionalReturn(value, !isArray ? _roundModifier(snapTo) : _isFunction(snapTo) ? function(raw) {
    is2D = snapTo(raw);
    return Math.abs(is2D - raw) <= radius ? is2D : raw;
  } : function(raw) {
    var x = parseFloat(is2D ? raw.x : raw), y = parseFloat(is2D ? raw.y : 0), min = _bigNum, closest = 0, i2 = snapTo.length, dx, dy;
    while (i2--) {
      if (is2D) {
        dx = snapTo[i2].x - x;
        dy = snapTo[i2].y - y;
        dx = dx * dx + dy * dy;
      } else {
        dx = Math.abs(snapTo[i2] - x);
      }
      if (dx < min) {
        min = dx;
        closest = i2;
      }
    }
    closest = !radius || min <= radius ? snapTo[closest] : raw;
    return is2D || closest === raw || _isNumber(raw) ? closest : closest + getUnit(raw);
  });
};
var random = function random2(min, max, roundingIncrement, returnFunction) {
  return _conditionalReturn(_isArray(min) ? !max : roundingIncrement === true ? !!(roundingIncrement = 0) : !returnFunction, function() {
    return _isArray(min) ? min[~~(Math.random() * min.length)] : (roundingIncrement = roundingIncrement || 1e-5) && (returnFunction = roundingIncrement < 1 ? Math.pow(10, (roundingIncrement + "").length - 2) : 1) && Math.floor(Math.round((min - roundingIncrement / 2 + Math.random() * (max - min + roundingIncrement * 0.99)) / roundingIncrement) * roundingIncrement * returnFunction) / returnFunction;
  });
};
var pipe = function pipe2() {
  for (var _len = arguments.length, functions = new Array(_len), _key = 0; _key < _len; _key++) {
    functions[_key] = arguments[_key];
  }
  return function(value) {
    return functions.reduce(function(v, f) {
      return f(v);
    }, value);
  };
};
var unitize = function unitize2(func, unit) {
  return function(value) {
    return func(parseFloat(value)) + (unit || getUnit(value));
  };
};
var normalize = function normalize2(min, max, value) {
  return mapRange(min, max, 0, 1, value);
};
var _wrapArray = function _wrapArray2(a, wrapper, value) {
  return _conditionalReturn(value, function(index) {
    return a[~~wrapper(index)];
  });
};
var wrap = function wrap2(min, max, value) {
  var range = max - min;
  return _isArray(min) ? _wrapArray(min, wrap2(0, min.length), max) : _conditionalReturn(value, function(value2) {
    return (range + (value2 - min) % range) % range + min;
  });
};
var wrapYoyo = function wrapYoyo2(min, max, value) {
  var range = max - min, total = range * 2;
  return _isArray(min) ? _wrapArray(min, wrapYoyo2(0, min.length - 1), max) : _conditionalReturn(value, function(value2) {
    value2 = (total + (value2 - min) % total) % total || 0;
    return min + (value2 > range ? total - value2 : value2);
  });
};
var _replaceRandom = function _replaceRandom2(s) {
  return s.replace(_randomExp, function(match) {
    var arIndex = match.indexOf("[") + 1, values = match.substring(arIndex || 7, arIndex ? match.indexOf("]") : match.length - 1).split(_commaDelimExp);
    return random(arIndex ? values : +values[0], arIndex ? 0 : +values[1], +values[2] || 1e-5);
  });
};
var mapRange = function mapRange2(inMin, inMax, outMin, outMax, value) {
  var inRange = inMax - inMin, outRange = outMax - outMin;
  return _conditionalReturn(value, function(value2) {
    return outMin + ((value2 - inMin) / inRange * outRange || 0);
  });
};
var interpolate = function interpolate2(start, end, progress, mutate) {
  var func = isNaN(start + end) ? 0 : function(p3) {
    return (1 - p3) * start + p3 * end;
  };
  if (!func) {
    var isString = _isString(start), master = {}, p2, i2, interpolators, l, il;
    progress === true && (mutate = 1) && (progress = null);
    if (isString) {
      start = {
        p: start
      };
      end = {
        p: end
      };
    } else if (_isArray(start) && !_isArray(end)) {
      interpolators = [];
      l = start.length;
      il = l - 2;
      for (i2 = 1; i2 < l; i2++) {
        interpolators.push(interpolate2(start[i2 - 1], start[i2]));
      }
      l--;
      func = function func2(p3) {
        p3 *= l;
        var i3 = Math.min(il, ~~p3);
        return interpolators[i3](p3 - i3);
      };
      progress = end;
    } else if (!mutate) {
      start = _merge(_isArray(start) ? [] : {}, start);
    }
    if (!interpolators) {
      for (p2 in end) {
        _addPropTween.call(master, start, p2, "get", end[p2]);
      }
      func = function func2(p3) {
        return _renderPropTweens(p3, master) || (isString ? start.p : start);
      };
    }
  }
  return _conditionalReturn(progress, func);
};
var _getLabelInDirection = function _getLabelInDirection2(timeline2, fromTime, backward) {
  var labels = timeline2.labels, min = _bigNum, p2, distance, label;
  for (p2 in labels) {
    distance = labels[p2] - fromTime;
    if (distance < 0 === !!backward && distance && min > (distance = Math.abs(distance))) {
      label = p2;
      min = distance;
    }
  }
  return label;
};
var _callback = function _callback2(animation, type, executeLazyFirst) {
  var v = animation.vars, callback = v[type], prevContext = _context, context3 = animation._ctx, params, scope, result;
  if (!callback) {
    return;
  }
  params = v[type + "Params"];
  scope = v.callbackScope || animation;
  executeLazyFirst && _lazyTweens.length && _lazyRender();
  context3 && (_context = context3);
  result = params ? callback.apply(scope, params) : callback.call(scope);
  _context = prevContext;
  return result;
};
var _interrupt = function _interrupt2(animation) {
  _removeFromParent(animation);
  animation.scrollTrigger && animation.scrollTrigger.kill(!!_reverting);
  animation.progress() < 1 && _callback(animation, "onInterrupt");
  return animation;
};
var _quickTween;
var _registerPluginQueue = [];
var _createPlugin = function _createPlugin2(config3) {
  if (!config3) return;
  config3 = !config3.name && config3["default"] || config3;
  if (_windowExists() || config3.headless) {
    var name = config3.name, isFunc = _isFunction(config3), Plugin = name && !isFunc && config3.init ? function() {
      this._props = [];
    } : config3, instanceDefaults = {
      init: _emptyFunc,
      render: _renderPropTweens,
      add: _addPropTween,
      kill: _killPropTweensOf,
      modifier: _addPluginModifier,
      rawVars: 0
    }, statics = {
      targetTest: 0,
      get: 0,
      getSetter: _getSetter,
      aliases: {},
      register: 0
    };
    _wake();
    if (config3 !== Plugin) {
      if (_plugins[name]) {
        return;
      }
      _setDefaults(Plugin, _setDefaults(_copyExcluding(config3, instanceDefaults), statics));
      _merge(Plugin.prototype, _merge(instanceDefaults, _copyExcluding(config3, statics)));
      _plugins[Plugin.prop = name] = Plugin;
      if (config3.targetTest) {
        _harnessPlugins.push(Plugin);
        _reservedProps[name] = 1;
      }
      name = (name === "css" ? "CSS" : name.charAt(0).toUpperCase() + name.substr(1)) + "Plugin";
    }
    _addGlobal(name, Plugin);
    config3.register && config3.register(gsap, Plugin, PropTween);
  } else {
    _registerPluginQueue.push(config3);
  }
};
var _255 = 255;
var _colorLookup = {
  aqua: [0, _255, _255],
  lime: [0, _255, 0],
  silver: [192, 192, 192],
  black: [0, 0, 0],
  maroon: [128, 0, 0],
  teal: [0, 128, 128],
  blue: [0, 0, _255],
  navy: [0, 0, 128],
  white: [_255, _255, _255],
  olive: [128, 128, 0],
  yellow: [_255, _255, 0],
  orange: [_255, 165, 0],
  gray: [128, 128, 128],
  purple: [128, 0, 128],
  green: [0, 128, 0],
  red: [_255, 0, 0],
  pink: [_255, 192, 203],
  cyan: [0, _255, _255],
  transparent: [_255, _255, _255, 0]
};
var _hue = function _hue2(h, m1, m2) {
  h += h < 0 ? 1 : h > 1 ? -1 : 0;
  return (h * 6 < 1 ? m1 + (m2 - m1) * h * 6 : h < 0.5 ? m2 : h * 3 < 2 ? m1 + (m2 - m1) * (2 / 3 - h) * 6 : m1) * _255 + 0.5 | 0;
};
var splitColor = function splitColor2(v, toHSL, forceAlpha) {
  var a = !v ? _colorLookup.black : _isNumber(v) ? [v >> 16, v >> 8 & _255, v & _255] : 0, r, g, b, h, s, l, max, min, d, wasHSL;
  if (!a) {
    if (v.substr(-1) === ",") {
      v = v.substr(0, v.length - 1);
    }
    if (_colorLookup[v]) {
      a = _colorLookup[v];
    } else if (v.charAt(0) === "#") {
      if (v.length < 6) {
        r = v.charAt(1);
        g = v.charAt(2);
        b = v.charAt(3);
        v = "#" + r + r + g + g + b + b + (v.length === 5 ? v.charAt(4) + v.charAt(4) : "");
      }
      if (v.length === 9) {
        a = parseInt(v.substr(1, 6), 16);
        return [a >> 16, a >> 8 & _255, a & _255, parseInt(v.substr(7), 16) / 255];
      }
      v = parseInt(v.substr(1), 16);
      a = [v >> 16, v >> 8 & _255, v & _255];
    } else if (v.substr(0, 3) === "hsl") {
      a = wasHSL = v.match(_strictNumExp);
      if (!toHSL) {
        h = +a[0] % 360 / 360;
        s = +a[1] / 100;
        l = +a[2] / 100;
        g = l <= 0.5 ? l * (s + 1) : l + s - l * s;
        r = l * 2 - g;
        a.length > 3 && (a[3] *= 1);
        a[0] = _hue(h + 1 / 3, r, g);
        a[1] = _hue(h, r, g);
        a[2] = _hue(h - 1 / 3, r, g);
      } else if (~v.indexOf("=")) {
        a = v.match(_numExp);
        forceAlpha && a.length < 4 && (a[3] = 1);
        return a;
      }
    } else {
      a = v.match(_strictNumExp) || _colorLookup.transparent;
    }
    a = a.map(Number);
  }
  if (toHSL && !wasHSL) {
    r = a[0] / _255;
    g = a[1] / _255;
    b = a[2] / _255;
    max = Math.max(r, g, b);
    min = Math.min(r, g, b);
    l = (max + min) / 2;
    if (max === min) {
      h = s = 0;
    } else {
      d = max - min;
      s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
      h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
      h *= 60;
    }
    a[0] = ~~(h + 0.5);
    a[1] = ~~(s * 100 + 0.5);
    a[2] = ~~(l * 100 + 0.5);
  }
  forceAlpha && a.length < 4 && (a[3] = 1);
  return a;
};
var _colorOrderData = function _colorOrderData2(v) {
  var values = [], c = [], i2 = -1;
  v.split(_colorExp).forEach(function(v2) {
    var a = v2.match(_numWithUnitExp) || [];
    values.push.apply(values, a);
    c.push(i2 += a.length + 1);
  });
  values.c = c;
  return values;
};
var _formatColors = function _formatColors2(s, toHSL, orderMatchData) {
  var result = "", colors = (s + result).match(_colorExp), type = toHSL ? "hsla(" : "rgba(", i2 = 0, c, shell, d, l;
  if (!colors) {
    return s;
  }
  colors = colors.map(function(color) {
    return (color = splitColor(color, toHSL, 1)) && type + (toHSL ? color[0] + "," + color[1] + "%," + color[2] + "%," + color[3] : color.join(",")) + ")";
  });
  if (orderMatchData) {
    d = _colorOrderData(s);
    c = orderMatchData.c;
    if (c.join(result) !== d.c.join(result)) {
      shell = s.replace(_colorExp, "1").split(_numWithUnitExp);
      l = shell.length - 1;
      for (; i2 < l; i2++) {
        result += shell[i2] + (~c.indexOf(i2) ? colors.shift() || type + "0,0,0,0)" : (d.length ? d : colors.length ? colors : orderMatchData).shift());
      }
    }
  }
  if (!shell) {
    shell = s.split(_colorExp);
    l = shell.length - 1;
    for (; i2 < l; i2++) {
      result += shell[i2] + colors[i2];
    }
  }
  return result + shell[l];
};
var _colorExp = (function() {
  var s = "(?:\\b(?:(?:rgb|rgba|hsl|hsla)\\(.+?\\))|\\B#(?:[0-9a-f]{3,4}){1,2}\\b", p2;
  for (p2 in _colorLookup) {
    s += "|" + p2 + "\\b";
  }
  return new RegExp(s + ")", "gi");
})();
var _hslExp = /hsl[a]?\(/;
var _colorStringFilter = function _colorStringFilter2(a) {
  var combined = a.join(" "), toHSL;
  _colorExp.lastIndex = 0;
  if (_colorExp.test(combined)) {
    toHSL = _hslExp.test(combined);
    a[1] = _formatColors(a[1], toHSL);
    a[0] = _formatColors(a[0], toHSL, _colorOrderData(a[1]));
    return true;
  }
};
var _tickerActive;
var _ticker = (function() {
  var _getTime5 = Date.now, _lagThreshold = 500, _adjustedLag = 33, _startTime = _getTime5(), _lastUpdate = _startTime, _gap = 1e3 / 240, _nextTime = _gap, _listeners3 = [], _id2, _req, _raf, _self, _delta, _i2, _tick = function _tick2(v) {
    var elapsed = _getTime5() - _lastUpdate, manual = v === true, overlap, dispatch, time, frame;
    (elapsed > _lagThreshold || elapsed < 0) && (_startTime += elapsed - _adjustedLag);
    _lastUpdate += elapsed;
    time = _lastUpdate - _startTime;
    overlap = time - _nextTime;
    if (overlap > 0 || manual) {
      frame = ++_self.frame;
      _delta = time - _self.time * 1e3;
      _self.time = time = time / 1e3;
      _nextTime += overlap + (overlap >= _gap ? 4 : _gap - overlap);
      dispatch = 1;
    }
    manual || (_id2 = _req(_tick2));
    if (dispatch) {
      for (_i2 = 0; _i2 < _listeners3.length; _i2++) {
        _listeners3[_i2](time, _delta, frame, v);
      }
    }
  };
  _self = {
    time: 0,
    frame: 0,
    tick: function tick() {
      _tick(true);
    },
    deltaRatio: function deltaRatio(fps) {
      return _delta / (1e3 / (fps || 60));
    },
    wake: function wake() {
      if (_coreReady) {
        if (!_coreInitted && _windowExists()) {
          _win = _coreInitted = window;
          _doc = _win.document || {};
          _globals.gsap = gsap;
          (_win.gsapVersions || (_win.gsapVersions = [])).push(gsap.version);
          _install(_installScope || _win.GreenSockGlobals || !_win.gsap && _win || {});
          _registerPluginQueue.forEach(_createPlugin);
        }
        _raf = typeof requestAnimationFrame !== "undefined" && requestAnimationFrame;
        _id2 && _self.sleep();
        _req = _raf || function(f) {
          return setTimeout(f, _nextTime - _self.time * 1e3 + 1 | 0);
        };
        _tickerActive = 1;
        _tick(2);
      }
    },
    sleep: function sleep() {
      (_raf ? cancelAnimationFrame : clearTimeout)(_id2);
      _tickerActive = 0;
      _req = _emptyFunc;
    },
    lagSmoothing: function lagSmoothing(threshold, adjustedLag) {
      _lagThreshold = threshold || Infinity;
      _adjustedLag = Math.min(adjustedLag || 33, _lagThreshold);
    },
    fps: function fps(_fps) {
      _gap = 1e3 / (_fps || 240);
      _nextTime = _self.time * 1e3 + _gap;
    },
    add: function add(callback, once, prioritize) {
      var func = once ? function(t, d, f, v) {
        callback(t, d, f, v);
        _self.remove(func);
      } : callback;
      _self.remove(callback);
      _listeners3[prioritize ? "unshift" : "push"](func);
      _wake();
      return func;
    },
    remove: function remove(callback, i2) {
      ~(i2 = _listeners3.indexOf(callback)) && _listeners3.splice(i2, 1) && _i2 >= i2 && _i2--;
    },
    _listeners: _listeners3
  };
  return _self;
})();
var _wake = function _wake2() {
  return !_tickerActive && _ticker.wake();
};
var _easeMap = {};
var _customEaseExp = /^[\d.\-M][\d.\-,\s]/;
var _quotesExp = /["']/g;
var _parseObjectInString = function _parseObjectInString2(value) {
  var obj = {}, split = value.substr(1, value.length - 3).split(":"), key = split[0], i2 = 1, l = split.length, index, val, parsedVal;
  for (; i2 < l; i2++) {
    val = split[i2];
    index = i2 !== l - 1 ? val.lastIndexOf(",") : val.length;
    parsedVal = val.substr(0, index);
    obj[key] = isNaN(parsedVal) ? parsedVal.replace(_quotesExp, "").trim() : +parsedVal;
    key = val.substr(index + 1).trim();
  }
  return obj;
};
var _valueInParentheses = function _valueInParentheses2(value) {
  var open = value.indexOf("(") + 1, close = value.indexOf(")"), nested = value.indexOf("(", open);
  return value.substring(open, ~nested && nested < close ? value.indexOf(")", close + 1) : close);
};
var _configEaseFromString = function _configEaseFromString2(name) {
  var split = (name + "").split("("), ease = _easeMap[split[0]];
  return ease && split.length > 1 && ease.config ? ease.config.apply(null, ~name.indexOf("{") ? [_parseObjectInString(split[1])] : _valueInParentheses(name).split(",").map(_numericIfPossible)) : _easeMap._CE && _customEaseExp.test(name) ? _easeMap._CE("", name) : ease;
};
var _invertEase = function _invertEase2(ease) {
  return function(p2) {
    return 1 - ease(1 - p2);
  };
};
var _parseEase = function _parseEase2(ease, defaultEase) {
  return !ease ? defaultEase : (_isFunction(ease) ? ease : _easeMap[ease] || _configEaseFromString(ease)) || defaultEase;
};
var _insertEase = function _insertEase2(names, easeIn, easeOut, easeInOut) {
  if (easeOut === void 0) {
    easeOut = function easeOut2(p2) {
      return 1 - easeIn(1 - p2);
    };
  }
  if (easeInOut === void 0) {
    easeInOut = function easeInOut2(p2) {
      return p2 < 0.5 ? easeIn(p2 * 2) / 2 : 1 - easeIn((1 - p2) * 2) / 2;
    };
  }
  var ease = {
    easeIn,
    easeOut,
    easeInOut
  }, lowercaseName;
  _forEachName(names, function(name) {
    _easeMap[name] = _globals[name] = ease;
    _easeMap[lowercaseName = name.toLowerCase()] = easeOut;
    for (var p2 in ease) {
      _easeMap[lowercaseName + (p2 === "easeIn" ? ".in" : p2 === "easeOut" ? ".out" : ".inOut")] = _easeMap[name + "." + p2] = ease[p2];
    }
  });
  return ease;
};
var _easeInOutFromOut = function _easeInOutFromOut2(easeOut) {
  return function(p2) {
    return p2 < 0.5 ? (1 - easeOut(1 - p2 * 2)) / 2 : 0.5 + easeOut((p2 - 0.5) * 2) / 2;
  };
};
var _configElastic = function _configElastic2(type, amplitude, period) {
  var p1 = amplitude >= 1 ? amplitude : 1, p2 = (period || (type ? 0.3 : 0.45)) / (amplitude < 1 ? amplitude : 1), p3 = p2 / _2PI * (Math.asin(1 / p1) || 0), easeOut = function easeOut2(p4) {
    return p4 === 1 ? 1 : p1 * Math.pow(2, -10 * p4) * _sin((p4 - p3) * p2) + 1;
  }, ease = type === "out" ? easeOut : type === "in" ? function(p4) {
    return 1 - easeOut(1 - p4);
  } : _easeInOutFromOut(easeOut);
  p2 = _2PI / p2;
  ease.config = function(amplitude2, period2) {
    return _configElastic2(type, amplitude2, period2);
  };
  return ease;
};
var _configBack = function _configBack2(type, overshoot) {
  if (overshoot === void 0) {
    overshoot = 1.70158;
  }
  var easeOut = function easeOut2(p2) {
    return p2 ? --p2 * p2 * ((overshoot + 1) * p2 + overshoot) + 1 : 0;
  }, ease = type === "out" ? easeOut : type === "in" ? function(p2) {
    return 1 - easeOut(1 - p2);
  } : _easeInOutFromOut(easeOut);
  ease.config = function(overshoot2) {
    return _configBack2(type, overshoot2);
  };
  return ease;
};
_forEachName("Linear,Quad,Cubic,Quart,Quint,Strong", function(name, i2) {
  var power = i2 < 5 ? i2 + 1 : i2;
  _insertEase(name + ",Power" + (power - 1), i2 ? function(p2) {
    return Math.pow(p2, power);
  } : function(p2) {
    return p2;
  }, function(p2) {
    return 1 - Math.pow(1 - p2, power);
  }, function(p2) {
    return p2 < 0.5 ? Math.pow(p2 * 2, power) / 2 : 1 - Math.pow((1 - p2) * 2, power) / 2;
  });
});
_easeMap.Linear.easeNone = _easeMap.none = _easeMap.Linear.easeIn;
_insertEase("Elastic", _configElastic("in"), _configElastic("out"), _configElastic());
(function(n, c) {
  var n1 = 1 / c, n2 = 2 * n1, n3 = 2.5 * n1, easeOut = function easeOut2(p2) {
    return p2 < n1 ? n * p2 * p2 : p2 < n2 ? n * Math.pow(p2 - 1.5 / c, 2) + 0.75 : p2 < n3 ? n * (p2 -= 2.25 / c) * p2 + 0.9375 : n * Math.pow(p2 - 2.625 / c, 2) + 0.984375;
  };
  _insertEase("Bounce", function(p2) {
    return 1 - easeOut(1 - p2);
  }, easeOut);
})(7.5625, 2.75);
_insertEase("Expo", function(p2) {
  return Math.pow(2, 10 * (p2 - 1)) * p2 + p2 * p2 * p2 * p2 * p2 * p2 * (1 - p2);
});
_insertEase("Circ", function(p2) {
  return -(_sqrt(1 - p2 * p2) - 1);
});
_insertEase("Sine", function(p2) {
  return p2 === 1 ? 1 : -_cos(p2 * _HALF_PI) + 1;
});
_insertEase("Back", _configBack("in"), _configBack("out"), _configBack());
_easeMap.SteppedEase = _easeMap.steps = _globals.SteppedEase = {
  config: function config(steps, immediateStart) {
    if (steps === void 0) {
      steps = 1;
    }
    var p1 = 1 / steps, p2 = steps + (immediateStart ? 0 : 1), p3 = immediateStart ? 1 : 0, max = 1 - _tinyNum;
    return function(p4) {
      return ((p2 * _clamp(0, max, p4) | 0) + p3) * p1;
    };
  }
};
_defaults.ease = _easeMap["quad.out"];
_forEachName("onComplete,onUpdate,onStart,onRepeat,onReverseComplete,onInterrupt", function(name) {
  return _callbackNames += name + "," + name + "Params,";
});
var GSCache = function GSCache2(target, harness) {
  this.id = _gsID++;
  target._gsap = this;
  this.target = target;
  this.harness = harness;
  this.get = harness ? harness.get : _getProperty;
  this.set = harness ? harness.getSetter : _getSetter;
};
var Animation = /* @__PURE__ */ (function() {
  function Animation3(vars) {
    this.vars = vars;
    this._delay = +vars.delay || 0;
    if (this._repeat = vars.repeat === Infinity ? -2 : vars.repeat || 0) {
      this._rDelay = vars.repeatDelay || 0;
      this._yoyo = !!vars.yoyo || !!vars.yoyoEase;
    }
    this._ts = 1;
    _setDuration(this, +vars.duration, 1, 1);
    this.data = vars.data;
    if (_context) {
      this._ctx = _context;
      _context.data.push(this);
    }
    _tickerActive || _ticker.wake();
  }
  var _proto = Animation3.prototype;
  _proto.delay = function delay(value) {
    if (value || value === 0) {
      this.parent && this.parent.smoothChildTiming && this.startTime(this._start + value - this._delay);
      this._delay = value;
      return this;
    }
    return this._delay;
  };
  _proto.duration = function duration(value) {
    return arguments.length ? this.totalDuration(this._repeat > 0 ? value + (value + this._rDelay) * this._repeat : value) : this.totalDuration() && this._dur;
  };
  _proto.totalDuration = function totalDuration(value) {
    if (!arguments.length) {
      return this._tDur;
    }
    this._dirty = 0;
    return _setDuration(this, this._repeat < 0 ? value : (value - this._repeat * this._rDelay) / (this._repeat + 1));
  };
  _proto.totalTime = function totalTime(_totalTime, suppressEvents) {
    _wake();
    if (!arguments.length) {
      return this._tTime;
    }
    var parent = this._dp;
    if (parent && parent.smoothChildTiming && this._ts) {
      _alignPlayhead(this, _totalTime);
      !parent._dp || parent.parent || _postAddChecks(parent, this);
      while (parent && parent.parent) {
        if (parent.parent._time !== parent._start + (parent._ts >= 0 ? parent._tTime / parent._ts : (parent.totalDuration() - parent._tTime) / -parent._ts)) {
          parent.totalTime(parent._tTime, true);
        }
        parent = parent.parent;
      }
      if (!this.parent && this._dp.autoRemoveChildren && (this._ts > 0 && _totalTime < this._tDur || this._ts < 0 && _totalTime > 0 || !this._tDur && !_totalTime)) {
        _addToTimeline(this._dp, this, this._start - this._delay);
      }
    }
    if (this._tTime !== _totalTime || !this._dur && !suppressEvents || this._initted && Math.abs(this._zTime) === _tinyNum || !this._initted && this._dur && _totalTime || !_totalTime && !this._initted && (this.add || this._ptLookup)) {
      this._ts || (this._pTime = _totalTime);
      _lazySafeRender(this, _totalTime, suppressEvents);
    }
    return this;
  };
  _proto.time = function time(value, suppressEvents) {
    return arguments.length ? this.totalTime(Math.min(this.totalDuration(), value + _elapsedCycleDuration(this)) % (this._dur + this._rDelay) || (value ? this._dur : 0), suppressEvents) : this._time;
  };
  _proto.totalProgress = function totalProgress(value, suppressEvents) {
    return arguments.length ? this.totalTime(this.totalDuration() * value, suppressEvents) : this.totalDuration() ? Math.min(1, this._tTime / this._tDur) : this.rawTime() >= 0 && this._initted ? 1 : 0;
  };
  _proto.progress = function progress(value, suppressEvents) {
    return arguments.length ? this.totalTime(this.duration() * (this._yoyo && !(this.iteration() & 1) ? 1 - value : value) + _elapsedCycleDuration(this), suppressEvents) : this.duration() ? Math.min(1, this._time / this._dur) : this.rawTime() > 0 ? 1 : 0;
  };
  _proto.iteration = function iteration(value, suppressEvents) {
    var cycleDuration = this.duration() + this._rDelay;
    return arguments.length ? this.totalTime(this._time + (value - 1) * cycleDuration, suppressEvents) : this._repeat ? _animationCycle(this._tTime, cycleDuration) + 1 : 1;
  };
  _proto.timeScale = function timeScale(value, suppressEvents) {
    if (!arguments.length) {
      return this._rts === -_tinyNum ? 0 : this._rts;
    }
    if (this._rts === value) {
      return this;
    }
    var tTime = this.parent && this._ts ? _parentToChildTotalTime(this.parent._time, this) : this._tTime;
    this._rts = +value || 0;
    this._ts = this._ps || value === -_tinyNum ? 0 : this._rts;
    this.totalTime(_clamp(-Math.abs(this._delay), this.totalDuration(), tTime), suppressEvents !== false);
    _setEnd(this);
    return _recacheAncestors(this);
  };
  _proto.paused = function paused(value) {
    if (!arguments.length) {
      return this._ps;
    }
    if (this._ps !== value) {
      this._ps = value;
      if (value) {
        this._pTime = this._tTime || Math.max(-this._delay, this.rawTime());
        this._ts = this._act = 0;
      } else {
        _wake();
        this._ts = this._rts;
        this.totalTime(this.parent && !this.parent.smoothChildTiming ? this.rawTime() : this._tTime || this._pTime, this.progress() === 1 && Math.abs(this._zTime) !== _tinyNum && (this._tTime -= _tinyNum));
      }
    }
    return this;
  };
  _proto.startTime = function startTime(value) {
    if (arguments.length) {
      this._start = _roundPrecise(value);
      var parent = this.parent || this._dp;
      parent && (parent._sort || !this.parent) && _addToTimeline(parent, this, this._start - this._delay);
      return this;
    }
    return this._start;
  };
  _proto.endTime = function endTime(includeRepeats) {
    return this._start + (_isNotFalse(includeRepeats) ? this.totalDuration() : this.duration()) / Math.abs(this._ts || 1);
  };
  _proto.rawTime = function rawTime(wrapRepeats) {
    var parent = this.parent || this._dp;
    return !parent ? this._tTime : wrapRepeats && (!this._ts || this._repeat && this._time && this.totalProgress() < 1) ? this._tTime % (this._dur + this._rDelay) : !this._ts ? this._tTime : _parentToChildTotalTime(parent.rawTime(wrapRepeats), this);
  };
  _proto.revert = function revert(config3) {
    if (config3 === void 0) {
      config3 = _revertConfig;
    }
    var prevIsReverting = _reverting;
    _reverting = config3;
    if (_isRevertWorthy(this)) {
      this.timeline && this.timeline.revert(config3);
      this.totalTime(-0.01, config3.suppressEvents);
    }
    this.data !== "nested" && config3.kill !== false && this.kill();
    _reverting = prevIsReverting;
    return this;
  };
  _proto.globalTime = function globalTime(rawTime) {
    var animation = this, time = arguments.length ? rawTime : animation.rawTime();
    while (animation) {
      time = animation._start + time / (Math.abs(animation._ts) || 1);
      animation = animation._dp;
    }
    return !this.parent && this._sat ? this._sat.globalTime(rawTime) : time;
  };
  _proto.repeat = function repeat(value) {
    if (arguments.length) {
      this._repeat = value === Infinity ? -2 : value;
      return _onUpdateTotalDuration(this);
    }
    return this._repeat === -2 ? Infinity : this._repeat;
  };
  _proto.repeatDelay = function repeatDelay(value) {
    if (arguments.length) {
      var time = this._time;
      this._rDelay = value;
      _onUpdateTotalDuration(this);
      return time ? this.time(time) : this;
    }
    return this._rDelay;
  };
  _proto.yoyo = function yoyo(value) {
    if (arguments.length) {
      this._yoyo = value;
      return this;
    }
    return this._yoyo;
  };
  _proto.seek = function seek(position, suppressEvents) {
    return this.totalTime(_parsePosition(this, position), _isNotFalse(suppressEvents));
  };
  _proto.restart = function restart(includeDelay, suppressEvents) {
    this.play().totalTime(includeDelay ? -this._delay : 0, _isNotFalse(suppressEvents));
    this._dur || (this._zTime = -_tinyNum);
    return this;
  };
  _proto.play = function play(from, suppressEvents) {
    from != null && this.seek(from, suppressEvents);
    return this.reversed(false).paused(false);
  };
  _proto.reverse = function reverse(from, suppressEvents) {
    from != null && this.seek(from || this.totalDuration(), suppressEvents);
    return this.reversed(true).paused(false);
  };
  _proto.pause = function pause(atTime, suppressEvents) {
    atTime != null && this.seek(atTime, suppressEvents);
    return this.paused(true);
  };
  _proto.resume = function resume() {
    return this.paused(false);
  };
  _proto.reversed = function reversed(value) {
    if (arguments.length) {
      !!value !== this.reversed() && this.timeScale(-this._rts || (value ? -_tinyNum : 0));
      return this;
    }
    return this._rts < 0;
  };
  _proto.invalidate = function invalidate() {
    this._initted = this._act = 0;
    this._zTime = -_tinyNum;
    return this;
  };
  _proto.isActive = function isActive() {
    var parent = this.parent || this._dp, start = this._start, rawTime;
    return !!(!parent || this._ts && this._initted && parent.isActive() && (rawTime = parent.rawTime(true)) >= start && rawTime < this.endTime(true) - _tinyNum);
  };
  _proto.eventCallback = function eventCallback(type, callback, params) {
    var vars = this.vars;
    if (arguments.length > 1) {
      if (!callback) {
        delete vars[type];
      } else {
        vars[type] = callback;
        params && (vars[type + "Params"] = params);
        type === "onUpdate" && (this._onUpdate = callback);
      }
      return this;
    }
    return vars[type];
  };
  _proto.then = function then(onFulfilled) {
    var self = this, prevProm = self._prom;
    return new Promise(function(resolve) {
      var f = _isFunction(onFulfilled) ? onFulfilled : _passThrough, _resolve = function _resolve2() {
        var _then = self.then;
        self.then = null;
        prevProm && prevProm();
        _isFunction(f) && (f = f(self)) && (f.then || f === self) && (self.then = _then);
        resolve(f);
        self.then = _then;
      };
      if (self._initted && self.totalProgress() === 1 && self._ts >= 0 || !self._tTime && self._ts < 0) {
        _resolve();
      } else {
        self._prom = _resolve;
      }
    });
  };
  _proto.kill = function kill5() {
    _interrupt(this);
  };
  return Animation3;
})();
_setDefaults(Animation.prototype, {
  _time: 0,
  _start: 0,
  _end: 0,
  _tTime: 0,
  _tDur: 0,
  _dirty: 0,
  _repeat: 0,
  _yoyo: false,
  parent: null,
  _initted: false,
  _rDelay: 0,
  _ts: 1,
  _dp: 0,
  ratio: 0,
  _zTime: -_tinyNum,
  _prom: 0,
  _ps: false,
  _rts: 1
});
var Timeline = /* @__PURE__ */ (function(_Animation) {
  _inheritsLoose(Timeline2, _Animation);
  function Timeline2(vars, position) {
    var _this;
    if (vars === void 0) {
      vars = {};
    }
    _this = _Animation.call(this, vars) || this;
    _this.labels = {};
    _this.smoothChildTiming = !!vars.smoothChildTiming;
    _this.autoRemoveChildren = !!vars.autoRemoveChildren;
    _this._sort = _isNotFalse(vars.sortChildren);
    _globalTimeline && _addToTimeline(vars.parent || _globalTimeline, _assertThisInitialized(_this), position);
    vars.reversed && _this.reverse();
    vars.paused && _this.paused(true);
    vars.scrollTrigger && _scrollTrigger(_assertThisInitialized(_this), vars.scrollTrigger);
    return _this;
  }
  var _proto2 = Timeline2.prototype;
  _proto2.to = function to(targets, vars, position) {
    _createTweenType(0, arguments, this);
    return this;
  };
  _proto2.from = function from(targets, vars, position) {
    _createTweenType(1, arguments, this);
    return this;
  };
  _proto2.fromTo = function fromTo(targets, fromVars, toVars, position) {
    _createTweenType(2, arguments, this);
    return this;
  };
  _proto2.set = function set(targets, vars, position) {
    vars.duration = 0;
    vars.parent = this;
    _inheritDefaults(vars).repeatDelay || (vars.repeat = 0);
    vars.immediateRender = !!vars.immediateRender;
    new Tween(targets, vars, _parsePosition(this, position), 1);
    return this;
  };
  _proto2.call = function call(callback, params, position) {
    return _addToTimeline(this, Tween.delayedCall(0, callback, params), position);
  };
  _proto2.staggerTo = function staggerTo(targets, duration, vars, stagger, position, onCompleteAll, onCompleteAllParams) {
    vars.duration = duration;
    vars.stagger = vars.stagger || stagger;
    vars.onComplete = onCompleteAll;
    vars.onCompleteParams = onCompleteAllParams;
    vars.parent = this;
    new Tween(targets, vars, _parsePosition(this, position));
    return this;
  };
  _proto2.staggerFrom = function staggerFrom(targets, duration, vars, stagger, position, onCompleteAll, onCompleteAllParams) {
    vars.runBackwards = 1;
    _inheritDefaults(vars).immediateRender = _isNotFalse(vars.immediateRender);
    return this.staggerTo(targets, duration, vars, stagger, position, onCompleteAll, onCompleteAllParams);
  };
  _proto2.staggerFromTo = function staggerFromTo(targets, duration, fromVars, toVars, stagger, position, onCompleteAll, onCompleteAllParams) {
    toVars.startAt = fromVars;
    _inheritDefaults(toVars).immediateRender = _isNotFalse(toVars.immediateRender);
    return this.staggerTo(targets, duration, toVars, stagger, position, onCompleteAll, onCompleteAllParams);
  };
  _proto2.render = function render14(totalTime, suppressEvents, force) {
    var prevTime = this._time, tDur = this._dirty ? this.totalDuration() : this._tDur, dur = this._dur, tTime = totalTime <= 0 ? 0 : _roundPrecise(totalTime), crossingStart = this._zTime < 0 !== totalTime < 0 && (this._initted || !dur), time, child, next, iteration, cycleDuration, prevPaused, pauseTween, timeScale, prevStart, prevIteration, yoyo, isYoyo;
    this !== _globalTimeline && tTime > tDur && totalTime >= 0 && (tTime = tDur);
    if (tTime !== this._tTime || force || crossingStart) {
      if (prevTime !== this._time && dur) {
        tTime += this._time - prevTime;
        totalTime += this._time - prevTime;
      }
      time = tTime;
      prevStart = this._start;
      timeScale = this._ts;
      prevPaused = !timeScale;
      if (crossingStart) {
        dur || (prevTime = this._zTime);
        (totalTime || !suppressEvents) && (this._zTime = totalTime);
      }
      if (this._repeat) {
        yoyo = this._yoyo;
        cycleDuration = dur + this._rDelay;
        if (this._repeat < -1 && totalTime < 0) {
          return this.totalTime(cycleDuration * 100 + totalTime, suppressEvents, force);
        }
        time = _roundPrecise(tTime % cycleDuration);
        if (tTime === tDur) {
          iteration = this._repeat;
          time = dur;
        } else {
          prevIteration = _roundPrecise(tTime / cycleDuration);
          iteration = ~~prevIteration;
          if (iteration && iteration === prevIteration) {
            time = dur;
            iteration--;
          }
          time > dur && (time = dur);
        }
        prevIteration = _animationCycle(this._tTime, cycleDuration);
        !prevTime && this._tTime && prevIteration !== iteration && this._tTime - prevIteration * cycleDuration - this._dur <= 0 && (prevIteration = iteration);
        if (yoyo && iteration & 1) {
          time = dur - time;
          isYoyo = 1;
        }
        if (iteration !== prevIteration && !this._lock) {
          var rewinding = yoyo && prevIteration & 1, doesWrap = rewinding === (yoyo && iteration & 1);
          iteration < prevIteration && (rewinding = !rewinding);
          prevTime = rewinding ? 0 : tTime % dur ? dur : tTime;
          this._lock = 1;
          this.render(prevTime || (isYoyo ? 0 : _roundPrecise(iteration * cycleDuration)), suppressEvents, !dur)._lock = 0;
          this._tTime = tTime;
          !suppressEvents && this.parent && _callback(this, "onRepeat");
          if (this.vars.repeatRefresh && !isYoyo) {
            this.invalidate()._lock = 1;
            prevIteration = iteration;
          }
          if (prevTime && prevTime !== this._time || prevPaused !== !this._ts || this.vars.onRepeat && !this.parent && !this._act) {
            return this;
          }
          dur = this._dur;
          tDur = this._tDur;
          if (doesWrap) {
            this._lock = 2;
            prevTime = rewinding ? dur : -1e-4;
            this.render(prevTime, true);
            this.vars.repeatRefresh && !isYoyo && this.invalidate();
          }
          this._lock = 0;
          if (!this._ts && !prevPaused) {
            return this;
          }
        }
      }
      if (this._hasPause && !this._forcing && this._lock < 2) {
        pauseTween = _findNextPauseTween(this, _roundPrecise(prevTime), _roundPrecise(time));
        if (pauseTween) {
          tTime -= time - (time = pauseTween._start);
        }
      }
      this._tTime = tTime;
      this._time = time;
      this._act = !!timeScale;
      if (!this._initted) {
        this._onUpdate = this.vars.onUpdate;
        this._initted = 1;
        this._zTime = totalTime;
        prevTime = 0;
      }
      if (!prevTime && tTime && dur && !suppressEvents && !prevIteration) {
        _callback(this, "onStart");
        if (this._tTime !== tTime) {
          return this;
        }
      }
      if (time >= prevTime && totalTime >= 0) {
        child = this._first;
        while (child) {
          next = child._next;
          if ((child._act || time >= child._start) && child._ts && pauseTween !== child) {
            if (child.parent !== this) {
              return this.render(totalTime, suppressEvents, force);
            }
            child.render(child._ts > 0 ? (time - child._start) * child._ts : (child._dirty ? child.totalDuration() : child._tDur) + (time - child._start) * child._ts, suppressEvents, force);
            if (time !== this._time || !this._ts && !prevPaused) {
              pauseTween = 0;
              next && (tTime += this._zTime = -_tinyNum);
              break;
            }
          }
          child = next;
        }
      } else {
        child = this._last;
        var adjustedTime = totalTime < 0 ? totalTime : time;
        while (child) {
          next = child._prev;
          if ((child._act || adjustedTime <= child._end) && child._ts && pauseTween !== child) {
            if (child.parent !== this) {
              return this.render(totalTime, suppressEvents, force);
            }
            child.render(child._ts > 0 ? (adjustedTime - child._start) * child._ts : (child._dirty ? child.totalDuration() : child._tDur) + (adjustedTime - child._start) * child._ts, suppressEvents, force || _reverting && _isRevertWorthy(child));
            if (time !== this._time || !this._ts && !prevPaused) {
              pauseTween = 0;
              next && (tTime += this._zTime = adjustedTime ? -_tinyNum : _tinyNum);
              break;
            }
          }
          child = next;
        }
      }
      if (pauseTween && !suppressEvents) {
        this.pause();
        pauseTween.render(time >= prevTime ? 0 : -_tinyNum)._zTime = time >= prevTime ? 1 : -1;
        if (this._ts) {
          this._start = prevStart;
          _setEnd(this);
          return this.render(totalTime, suppressEvents, force);
        }
      }
      this._onUpdate && !suppressEvents && _callback(this, "onUpdate", true);
      if (tTime === tDur && this._tTime >= this.totalDuration() || !tTime && prevTime) {
        if (prevStart === this._start || Math.abs(timeScale) !== Math.abs(this._ts)) {
          if (!this._lock) {
            (totalTime || !dur) && (tTime === tDur && this._ts > 0 || !tTime && this._ts < 0) && _removeFromParent(this, 1);
            if (!suppressEvents && !(totalTime < 0 && !prevTime) && (tTime || prevTime || !tDur)) {
              _callback(this, tTime === tDur && totalTime >= 0 ? "onComplete" : "onReverseComplete", true);
              this._prom && !(tTime < tDur && this.timeScale() > 0) && this._prom();
            }
          }
        }
      }
    }
    return this;
  };
  _proto2.add = function add(child, position) {
    var _this2 = this;
    _isNumber(position) || (position = _parsePosition(this, position, child));
    if (!(child instanceof Animation)) {
      if (_isArray(child)) {
        child.forEach(function(obj) {
          return _this2.add(obj, position);
        });
        return this;
      }
      if (_isString(child)) {
        return this.addLabel(child, position);
      }
      if (_isFunction(child)) {
        child = Tween.delayedCall(0, child);
      } else {
        return this;
      }
    }
    return this !== child ? _addToTimeline(this, child, position) : this;
  };
  _proto2.getChildren = function getChildren(nested, tweens, timelines, ignoreBeforeTime) {
    if (nested === void 0) {
      nested = true;
    }
    if (tweens === void 0) {
      tweens = true;
    }
    if (timelines === void 0) {
      timelines = true;
    }
    if (ignoreBeforeTime === void 0) {
      ignoreBeforeTime = -_bigNum;
    }
    var a = [], child = this._first;
    while (child) {
      if (child._start >= ignoreBeforeTime) {
        if (child instanceof Tween) {
          tweens && a.push(child);
        } else {
          timelines && a.push(child);
          nested && a.push.apply(a, child.getChildren(true, tweens, timelines));
        }
      }
      child = child._next;
    }
    return a;
  };
  _proto2.getById = function getById2(id) {
    var animations = this.getChildren(1, 1, 1), i2 = animations.length;
    while (i2--) {
      if (animations[i2].vars.id === id) {
        return animations[i2];
      }
    }
  };
  _proto2.remove = function remove(child) {
    if (_isString(child)) {
      return this.removeLabel(child);
    }
    if (_isFunction(child)) {
      return this.killTweensOf(child);
    }
    child.parent === this && _removeLinkedListItem(this, child);
    if (child === this._recent) {
      this._recent = this._last;
    }
    return _uncache(this);
  };
  _proto2.totalTime = function totalTime(_totalTime2, suppressEvents) {
    if (!arguments.length) {
      return this._tTime;
    }
    this._forcing = 1;
    if (!this._dp && this._ts) {
      this._start = _roundPrecise(_ticker.time - (this._ts > 0 ? _totalTime2 / this._ts : (this.totalDuration() - _totalTime2) / -this._ts));
    }
    _Animation.prototype.totalTime.call(this, _totalTime2, suppressEvents);
    this._forcing = 0;
    return this;
  };
  _proto2.addLabel = function addLabel(label, position) {
    this.labels[label] = _parsePosition(this, position);
    return this;
  };
  _proto2.removeLabel = function removeLabel(label) {
    delete this.labels[label];
    return this;
  };
  _proto2.addPause = function addPause(position, callback, params) {
    var t = Tween.delayedCall(0, callback || _emptyFunc, params);
    t.data = "isPause";
    this._hasPause = 1;
    return _addToTimeline(this, t, _parsePosition(this, position));
  };
  _proto2.removePause = function removePause(position) {
    var child = this._first;
    position = _parsePosition(this, position);
    while (child) {
      if (child._start === position && child.data === "isPause") {
        _removeFromParent(child);
      }
      child = child._next;
    }
  };
  _proto2.killTweensOf = function killTweensOf(targets, props, onlyActive) {
    var tweens = this.getTweensOf(targets, onlyActive), i2 = tweens.length;
    while (i2--) {
      _overwritingTween !== tweens[i2] && tweens[i2].kill(targets, props);
    }
    return this;
  };
  _proto2.getTweensOf = function getTweensOf2(targets, onlyActive) {
    var a = [], parsedTargets = toArray(targets), child = this._first, isGlobalTime = _isNumber(onlyActive), children;
    while (child) {
      if (child instanceof Tween) {
        if (_arrayContainsAny(child._targets, parsedTargets) && (isGlobalTime ? (!_overwritingTween || child._initted && child._ts) && child.globalTime(0) <= onlyActive && child.globalTime(child.totalDuration()) > onlyActive : !onlyActive || child.isActive())) {
          a.push(child);
        }
      } else if ((children = child.getTweensOf(parsedTargets, onlyActive)).length) {
        a.push.apply(a, children);
      }
      child = child._next;
    }
    return a;
  };
  _proto2.tweenTo = function tweenTo(position, vars) {
    vars = vars || {};
    var tl = this, endTime = _parsePosition(tl, position), _vars = vars, startAt = _vars.startAt, _onStart = _vars.onStart, onStartParams = _vars.onStartParams, immediateRender = _vars.immediateRender, initted, tween = Tween.to(tl, _setDefaults({
      ease: vars.ease || "none",
      lazy: false,
      immediateRender: false,
      time: endTime,
      overwrite: "auto",
      duration: vars.duration || Math.abs((endTime - (startAt && "time" in startAt ? startAt.time : tl._time)) / tl.timeScale()) || _tinyNum,
      onStart: function onStart() {
        tl.pause();
        if (!initted) {
          var duration = vars.duration || Math.abs((endTime - (startAt && "time" in startAt ? startAt.time : tl._time)) / tl.timeScale());
          tween._dur !== duration && _setDuration(tween, duration, 0, 1).render(tween._time, true, true);
          initted = 1;
        }
        _onStart && _onStart.apply(tween, onStartParams || []);
      }
    }, vars));
    return immediateRender ? tween.render(0) : tween;
  };
  _proto2.tweenFromTo = function tweenFromTo(fromPosition, toPosition, vars) {
    return this.tweenTo(toPosition, _setDefaults({
      startAt: {
        time: _parsePosition(this, fromPosition)
      }
    }, vars));
  };
  _proto2.recent = function recent() {
    return this._recent;
  };
  _proto2.nextLabel = function nextLabel(afterTime) {
    if (afterTime === void 0) {
      afterTime = this._time;
    }
    return _getLabelInDirection(this, _parsePosition(this, afterTime));
  };
  _proto2.previousLabel = function previousLabel(beforeTime) {
    if (beforeTime === void 0) {
      beforeTime = this._time;
    }
    return _getLabelInDirection(this, _parsePosition(this, beforeTime), 1);
  };
  _proto2.currentLabel = function currentLabel(value) {
    return arguments.length ? this.seek(value, true) : this.previousLabel(this._time + _tinyNum);
  };
  _proto2.shiftChildren = function shiftChildren(amount, adjustLabels, ignoreBeforeTime) {
    if (ignoreBeforeTime === void 0) {
      ignoreBeforeTime = 0;
    }
    var child = this._first, labels = this.labels, p2;
    amount = _roundPrecise(amount);
    while (child) {
      if (child._start >= ignoreBeforeTime) {
        child._start += amount;
        child._end += amount;
      }
      child = child._next;
    }
    if (adjustLabels) {
      for (p2 in labels) {
        if (labels[p2] >= ignoreBeforeTime) {
          labels[p2] += amount;
        }
      }
    }
    return _uncache(this);
  };
  _proto2.invalidate = function invalidate(soft) {
    var child = this._first;
    this._lock = 0;
    while (child) {
      child.invalidate(soft);
      child = child._next;
    }
    return _Animation.prototype.invalidate.call(this, soft);
  };
  _proto2.clear = function clear(includeLabels) {
    if (includeLabels === void 0) {
      includeLabels = true;
    }
    var child = this._first, next;
    while (child) {
      next = child._next;
      this.remove(child);
      child = next;
    }
    this._dp && (this._time = this._tTime = this._pTime = 0);
    includeLabels && (this.labels = {});
    return _uncache(this);
  };
  _proto2.totalDuration = function totalDuration(value) {
    var max = 0, self = this, child = self._last, prevStart = _bigNum, prev, start, parent;
    if (arguments.length) {
      return self.timeScale((self._repeat < 0 ? self.duration() : self.totalDuration()) / (self.reversed() ? -value : value));
    }
    if (self._dirty) {
      parent = self.parent;
      while (child) {
        prev = child._prev;
        child._dirty && child.totalDuration();
        start = child._start;
        if (start > prevStart && self._sort && child._ts && !self._lock) {
          self._lock = 1;
          _addToTimeline(self, child, start - child._delay, 1)._lock = 0;
        } else {
          prevStart = start;
        }
        if (start < 0 && child._ts) {
          max -= start;
          if (!parent && !self._dp || parent && parent.smoothChildTiming) {
            self._start += _roundPrecise(start / self._ts);
            self._time -= start;
            self._tTime -= start;
          }
          self.shiftChildren(-start, false, -Infinity);
          prevStart = 0;
        }
        child._end > max && child._ts && (max = child._end);
        child = prev;
      }
      _setDuration(self, self === _globalTimeline && self._time > max ? self._time : max, 1, 1);
      self._dirty = 0;
    }
    return self._tDur;
  };
  Timeline2.updateRoot = function updateRoot(time) {
    if (_globalTimeline._ts) {
      _lazySafeRender(_globalTimeline, _parentToChildTotalTime(time, _globalTimeline));
      _lastRenderedFrame = _ticker.frame;
    }
    if (_ticker.frame >= _nextGCFrame) {
      _nextGCFrame += _config.autoSleep || 120;
      var child = _globalTimeline._first;
      if (!child || !child._ts) {
        if (_config.autoSleep && _ticker._listeners.length < 2) {
          while (child && !child._ts) {
            child = child._next;
          }
          child || _ticker.sleep();
        }
      }
    }
  };
  return Timeline2;
})(Animation);
_setDefaults(Timeline.prototype, {
  _lock: 0,
  _hasPause: 0,
  _forcing: 0
});
var _addComplexStringPropTween = function _addComplexStringPropTween2(target, prop, start, end, setter, stringFilter, funcParam) {
  var pt = new PropTween(this._pt, target, prop, 0, 1, _renderComplexString, null, setter), index = 0, matchIndex = 0, result, startNums, color, endNum, chunk, startNum, hasRandom, a;
  pt.b = start;
  pt.e = end;
  start += "";
  end += "";
  if (hasRandom = ~end.indexOf("random(")) {
    end = _replaceRandom(end);
  }
  if (stringFilter) {
    a = [start, end];
    stringFilter(a, target, prop);
    start = a[0];
    end = a[1];
  }
  startNums = start.match(_complexStringNumExp) || [];
  while (result = _complexStringNumExp.exec(end)) {
    endNum = result[0];
    chunk = end.substring(index, result.index);
    if (color) {
      color = (color + 1) % 5;
    } else if (chunk.substr(-5) === "rgba(") {
      color = 1;
    }
    if (endNum !== startNums[matchIndex++]) {
      startNum = parseFloat(startNums[matchIndex - 1]) || 0;
      pt._pt = {
        _next: pt._pt,
        p: chunk || matchIndex === 1 ? chunk : ",",
        //note: SVG spec allows omission of comma/space when a negative sign is wedged between two numbers, like 2.5-5.3 instead of 2.5,-5.3 but when tweening, the negative value may switch to positive, so we insert the comma just in case.
        s: startNum,
        c: endNum.charAt(1) === "=" ? _parseRelative(startNum, endNum) - startNum : parseFloat(endNum) - startNum,
        m: color && color < 4 ? Math.round : 0
      };
      index = _complexStringNumExp.lastIndex;
    }
  }
  pt.c = index < end.length ? end.substring(index, end.length) : "";
  pt.fp = funcParam;
  if (_relExp.test(end) || hasRandom) {
    pt.e = 0;
  }
  this._pt = pt;
  return pt;
};
var _addPropTween = function _addPropTween2(target, prop, start, end, index, targets, modifier, stringFilter, funcParam, optional) {
  _isFunction(end) && (end = end(index || 0, target, targets));
  var currentValue = target[prop], parsedStart = start !== "get" ? start : !_isFunction(currentValue) ? currentValue : funcParam ? target[prop.indexOf("set") || !_isFunction(target["get" + prop.substr(3)]) ? prop : "get" + prop.substr(3)](funcParam) : target[prop](), setter = !_isFunction(currentValue) ? _setterPlain : funcParam ? _setterFuncWithParam : _setterFunc, pt;
  if (_isString(end)) {
    if (~end.indexOf("random(")) {
      end = _replaceRandom(end);
    }
    if (end.charAt(1) === "=") {
      pt = _parseRelative(parsedStart, end) + (getUnit(parsedStart) || 0);
      if (pt || pt === 0) {
        end = pt;
      }
    }
  }
  if (!optional || parsedStart !== end || _forceAllPropTweens) {
    if (!isNaN(parsedStart * end) && end !== "") {
      pt = new PropTween(this._pt, target, prop, +parsedStart || 0, end - (parsedStart || 0), typeof currentValue === "boolean" ? _renderBoolean : _renderPlain, 0, setter);
      funcParam && (pt.fp = funcParam);
      modifier && pt.modifier(modifier, this, target);
      return this._pt = pt;
    }
    !currentValue && !(prop in target) && _missingPlugin(prop, end);
    return _addComplexStringPropTween.call(this, target, prop, parsedStart, end, setter, stringFilter || _config.stringFilter, funcParam);
  }
};
var _processVars = function _processVars2(vars, index, target, targets, tween) {
  _isFunction(vars) && (vars = _parseFuncOrString(vars, tween, index, target, targets));
  if (!_isObject(vars) || vars.style && vars.nodeType || _isArray(vars) || _isTypedArray(vars)) {
    return _isString(vars) ? _parseFuncOrString(vars, tween, index, target, targets) : vars;
  }
  var copy = {}, p2;
  for (p2 in vars) {
    copy[p2] = _parseFuncOrString(vars[p2], tween, index, target, targets);
  }
  return copy;
};
var _checkPlugin = function _checkPlugin2(property, vars, tween, index, target, targets) {
  var plugin, pt, ptLookup, i2;
  if (_plugins[property] && (plugin = new _plugins[property]()).init(target, plugin.rawVars ? vars[property] : _processVars(vars[property], index, target, targets, tween), tween, index, targets) !== false) {
    tween._pt = pt = new PropTween(tween._pt, target, property, 0, 1, plugin.render, plugin, 0, plugin.priority);
    if (tween !== _quickTween) {
      ptLookup = tween._ptLookup[tween._targets.indexOf(target)];
      i2 = plugin._props.length;
      while (i2--) {
        ptLookup[plugin._props[i2]] = pt;
      }
    }
  }
  return plugin;
};
var _overwritingTween;
var _forceAllPropTweens;
var _initTween = function _initTween2(tween, time, tTime) {
  var vars = tween.vars, ease = vars.ease, startAt = vars.startAt, immediateRender = vars.immediateRender, lazy = vars.lazy, onUpdate = vars.onUpdate, runBackwards = vars.runBackwards, yoyoEase = vars.yoyoEase, keyframes = vars.keyframes, autoRevert = vars.autoRevert, dur = tween._dur, prevStartAt = tween._startAt, targets = tween._targets, parent = tween.parent, fullTargets = parent && parent.data === "nested" ? parent.vars.targets : targets, autoOverwrite = tween._overwrite === "auto" && !_suppressOverwrites, tl = tween.timeline, reverseEase = vars.easeReverse || yoyoEase, cleanVars, i2, p2, pt, target, hasPriority, gsData, harness, plugin, ptLookup, index, harnessVars, overwritten;
  tl && (!keyframes || !ease) && (ease = "none");
  tween._ease = _parseEase(ease, _defaults.ease);
  tween._rEase = reverseEase && (_parseEase(reverseEase) || tween._ease);
  tween._from = !tl && !!vars.runBackwards;
  if (tween._from) tween.ratio = 1;
  if (!tl || keyframes && !vars.stagger) {
    harness = targets[0] ? _getCache(targets[0]).harness : 0;
    harnessVars = harness && vars[harness.prop];
    cleanVars = _copyExcluding(vars, _reservedProps);
    if (prevStartAt) {
      prevStartAt._zTime < 0 && prevStartAt.progress(1);
      time < 0 && runBackwards && immediateRender && !autoRevert ? prevStartAt.render(-1, true) : prevStartAt.revert(runBackwards && dur ? _revertConfigNoKill : _startAtRevertConfig);
      prevStartAt._lazy = 0;
    }
    if (startAt) {
      _removeFromParent(tween._startAt = Tween.set(targets, _setDefaults({
        data: "isStart",
        overwrite: false,
        parent,
        immediateRender: true,
        lazy: !prevStartAt && _isNotFalse(lazy),
        startAt: null,
        delay: 0,
        onUpdate: onUpdate && function() {
          return _callback(tween, "onUpdate");
        },
        stagger: 0
      }, startAt)));
      tween._startAt._dp = 0;
      tween._startAt._sat = tween;
      time < 0 && (_reverting || !immediateRender && !autoRevert) && tween._startAt.revert(_revertConfigNoKill);
      if (immediateRender) {
        if (dur && time <= 0 && tTime <= 0) {
          time && (tween._zTime = time);
          return;
        }
      }
    } else if (runBackwards && dur) {
      if (!prevStartAt) {
        time && (immediateRender = false);
        p2 = _setDefaults({
          overwrite: false,
          data: "isFromStart",
          //we tag the tween with as "isFromStart" so that if [inside a plugin] we need to only do something at the very END of a tween, we have a way of identifying this tween as merely the one that's setting the beginning values for a "from()" tween. For example, clearProps in CSSPlugin should only get applied at the very END of a tween and without this tag, from(...{height:100, clearProps:"height", delay:1}) would wipe the height at the beginning of the tween and after 1 second, it'd kick back in.
          lazy: immediateRender && !prevStartAt && _isNotFalse(lazy),
          immediateRender,
          //zero-duration tweens render immediately by default, but if we're not specifically instructed to render this tween immediately, we should skip this and merely _init() to record the starting values (rendering them immediately would push them to completion which is wasteful in that case - we'd have to render(-1) immediately after)
          stagger: 0,
          parent
          //ensures that nested tweens that had a stagger are handled properly, like gsap.from(".class", {y: gsap.utils.wrap([-100,100]), stagger: 0.5})
        }, cleanVars);
        harnessVars && (p2[harness.prop] = harnessVars);
        _removeFromParent(tween._startAt = Tween.set(targets, p2));
        tween._startAt._dp = 0;
        tween._startAt._sat = tween;
        time < 0 && (_reverting ? tween._startAt.revert(_revertConfigNoKill) : tween._startAt.render(-1, true));
        tween._zTime = time;
        if (!immediateRender) {
          _initTween2(tween._startAt, _tinyNum, _tinyNum);
        } else if (!time) {
          return;
        }
      }
    }
    tween._pt = tween._ptCache = 0;
    lazy = dur && _isNotFalse(lazy) || lazy && !dur;
    for (i2 = 0; i2 < targets.length; i2++) {
      target = targets[i2];
      gsData = target._gsap || _harness(targets)[i2]._gsap;
      tween._ptLookup[i2] = ptLookup = {};
      _lazyLookup[gsData.id] && _lazyTweens.length && _lazyRender();
      index = fullTargets === targets ? i2 : fullTargets.indexOf(target);
      if (harness && (plugin = new harness()).init(target, harnessVars || cleanVars, tween, index, fullTargets) !== false) {
        tween._pt = pt = new PropTween(tween._pt, target, plugin.name, 0, 1, plugin.render, plugin, 0, plugin.priority);
        plugin._props.forEach(function(name) {
          ptLookup[name] = pt;
        });
        plugin.priority && (hasPriority = 1);
      }
      if (!harness || harnessVars) {
        for (p2 in cleanVars) {
          if (_plugins[p2] && (plugin = _checkPlugin(p2, cleanVars, tween, index, target, fullTargets))) {
            plugin.priority && (hasPriority = 1);
          } else {
            ptLookup[p2] = pt = _addPropTween.call(tween, target, p2, "get", cleanVars[p2], index, fullTargets, 0, vars.stringFilter);
          }
        }
      }
      tween._op && tween._op[i2] && tween.kill(target, tween._op[i2]);
      if (autoOverwrite && tween._pt) {
        _overwritingTween = tween;
        _globalTimeline.killTweensOf(target, ptLookup, tween.globalTime(time));
        overwritten = !tween.parent;
        _overwritingTween = 0;
      }
      tween._pt && lazy && (_lazyLookup[gsData.id] = 1);
    }
    hasPriority && _sortPropTweensByPriority(tween);
    tween._onInit && tween._onInit(tween);
  }
  tween._onUpdate = onUpdate;
  tween._initted = (!tween._op || tween._pt) && !overwritten;
  keyframes && time <= 0 && tl.render(_bigNum, true, true);
};
var _updatePropTweens = function _updatePropTweens2(tween, property, value, start, startIsRelative, ratio, time, skipRecursion) {
  var ptCache = (tween._pt && tween._ptCache || (tween._ptCache = {}))[property], pt, rootPT, lookup, i2;
  if (!ptCache) {
    ptCache = tween._ptCache[property] = [];
    lookup = tween._ptLookup;
    i2 = tween._targets.length;
    while (i2--) {
      pt = lookup[i2][property];
      if (pt && pt.d && pt.d._pt) {
        pt = pt.d._pt;
        while (pt && pt.p !== property && pt.fp !== property) {
          pt = pt._next;
        }
      }
      if (!pt) {
        _forceAllPropTweens = 1;
        tween.vars[property] = "+=0";
        _initTween(tween, time);
        _forceAllPropTweens = 0;
        return skipRecursion ? _warn(property + " not eligible for reset. Try splitting into individual properties") : 1;
      }
      ptCache.push(pt);
    }
  }
  i2 = ptCache.length;
  while (i2--) {
    rootPT = ptCache[i2];
    pt = rootPT._pt || rootPT;
    pt.s = (start || start === 0) && !startIsRelative ? start : pt.s + (start || 0) + ratio * pt.c;
    pt.c = value - pt.s;
    rootPT.e && (rootPT.e = _round(value) + getUnit(rootPT.e));
    rootPT.b && (rootPT.b = pt.s + getUnit(rootPT.b));
  }
};
var _addAliasesToVars = function _addAliasesToVars2(targets, vars) {
  var harness = targets[0] ? _getCache(targets[0]).harness : 0, propertyAliases = harness && harness.aliases, copy, p2, i2, aliases;
  if (!propertyAliases) {
    return vars;
  }
  copy = _merge({}, vars);
  for (p2 in propertyAliases) {
    if (p2 in copy) {
      aliases = propertyAliases[p2].split(",");
      i2 = aliases.length;
      while (i2--) {
        copy[aliases[i2]] = copy[p2];
      }
    }
  }
  return copy;
};
var _parseKeyframe = function _parseKeyframe2(prop, obj, allProps, easeEach) {
  var ease = obj.ease || easeEach || "power1.inOut", p2, a;
  if (_isArray(obj)) {
    a = allProps[prop] || (allProps[prop] = []);
    obj.forEach(function(value, i2) {
      return a.push({
        t: i2 / (obj.length - 1) * 100,
        v: value,
        e: ease
      });
    });
  } else {
    for (p2 in obj) {
      a = allProps[p2] || (allProps[p2] = []);
      p2 === "ease" || a.push({
        t: parseFloat(prop),
        v: obj[p2],
        e: ease
      });
    }
  }
};
var _parseFuncOrString = function _parseFuncOrString2(value, tween, i2, target, targets) {
  return _isFunction(value) ? value.call(tween, i2, target, targets) : _isString(value) && ~value.indexOf("random(") ? _replaceRandom(value) : value;
};
var _staggerTweenProps = _callbackNames + "repeat,repeatDelay,yoyo,repeatRefresh,yoyoEase,easeReverse,autoRevert";
var _staggerPropsToSkip = {};
_forEachName(_staggerTweenProps + ",id,stagger,delay,duration,paused,scrollTrigger", function(name) {
  return _staggerPropsToSkip[name] = 1;
});
var Tween = /* @__PURE__ */ (function(_Animation2) {
  _inheritsLoose(Tween2, _Animation2);
  function Tween2(targets, vars, position, skipInherit) {
    var _this3;
    if (typeof vars === "number") {
      position.duration = vars;
      vars = position;
      position = null;
    }
    _this3 = _Animation2.call(this, skipInherit ? vars : _inheritDefaults(vars)) || this;
    var _this3$vars = _this3.vars, duration = _this3$vars.duration, delay = _this3$vars.delay, immediateRender = _this3$vars.immediateRender, stagger = _this3$vars.stagger, overwrite = _this3$vars.overwrite, keyframes = _this3$vars.keyframes, defaults2 = _this3$vars.defaults, scrollTrigger = _this3$vars.scrollTrigger, parent = vars.parent || _globalTimeline, parsedTargets = (_isArray(targets) || _isTypedArray(targets) ? _isNumber(targets[0]) : "length" in vars) ? [targets] : toArray(targets), tl, i2, copy, l, p2, curTarget, staggerFunc, staggerVarsToMerge;
    _this3._targets = parsedTargets.length ? _harness(parsedTargets) : _warn("GSAP target " + targets + " not found. https://gsap.com", !_config.nullTargetWarn) || [];
    _this3._ptLookup = [];
    _this3._overwrite = overwrite;
    if (keyframes || stagger || _isFuncOrString(duration) || _isFuncOrString(delay)) {
      vars = _this3.vars;
      var easeReverse = vars.easeReverse || vars.yoyoEase;
      tl = _this3.timeline = new Timeline({
        data: "nested",
        defaults: defaults2 || {},
        targets: parent && parent.data === "nested" ? parent.vars.targets : parsedTargets
      });
      tl.kill();
      tl.parent = tl._dp = _assertThisInitialized(_this3);
      tl._start = 0;
      if (stagger || _isFuncOrString(duration) || _isFuncOrString(delay)) {
        l = parsedTargets.length;
        staggerFunc = stagger && distribute(stagger);
        if (_isObject(stagger)) {
          for (p2 in stagger) {
            if (~_staggerTweenProps.indexOf(p2)) {
              staggerVarsToMerge || (staggerVarsToMerge = {});
              staggerVarsToMerge[p2] = stagger[p2];
            }
          }
        }
        for (i2 = 0; i2 < l; i2++) {
          copy = _copyExcluding(vars, _staggerPropsToSkip);
          copy.stagger = 0;
          easeReverse && (copy.easeReverse = easeReverse);
          staggerVarsToMerge && _merge(copy, staggerVarsToMerge);
          curTarget = parsedTargets[i2];
          copy.duration = +_parseFuncOrString(duration, _assertThisInitialized(_this3), i2, curTarget, parsedTargets);
          copy.delay = (+_parseFuncOrString(delay, _assertThisInitialized(_this3), i2, curTarget, parsedTargets) || 0) - _this3._delay;
          if (!stagger && l === 1 && copy.delay) {
            _this3._delay = delay = copy.delay;
            _this3._start += delay;
            copy.delay = 0;
          }
          tl.to(curTarget, copy, staggerFunc ? staggerFunc(i2, curTarget, parsedTargets) : 0);
          tl._ease = _easeMap.none;
        }
        tl.duration() ? duration = delay = 0 : _this3.timeline = 0;
      } else if (keyframes) {
        _inheritDefaults(_setDefaults(tl.vars.defaults, {
          ease: "none"
        }));
        tl._ease = _parseEase(keyframes.ease || vars.ease || "none");
        var time = 0, a, kf, v;
        if (_isArray(keyframes)) {
          keyframes.forEach(function(frame) {
            return tl.to(parsedTargets, frame, ">");
          });
          tl.duration();
        } else {
          copy = {};
          for (p2 in keyframes) {
            p2 === "ease" || p2 === "easeEach" || _parseKeyframe(p2, keyframes[p2], copy, keyframes.easeEach);
          }
          for (p2 in copy) {
            a = copy[p2].sort(function(a2, b) {
              return a2.t - b.t;
            });
            time = 0;
            for (i2 = 0; i2 < a.length; i2++) {
              kf = a[i2];
              v = {
                ease: kf.e,
                duration: (kf.t - (i2 ? a[i2 - 1].t : 0)) / 100 * duration
              };
              v[p2] = kf.v;
              tl.to(parsedTargets, v, time);
              time += v.duration;
            }
          }
          tl.duration() < duration && tl.to({}, {
            duration: duration - tl.duration()
          });
        }
      }
      duration || _this3.duration(duration = tl.duration());
    } else {
      _this3.timeline = 0;
    }
    if (overwrite === true && !_suppressOverwrites) {
      _overwritingTween = _assertThisInitialized(_this3);
      _globalTimeline.killTweensOf(parsedTargets);
      _overwritingTween = 0;
    }
    _addToTimeline(parent, _assertThisInitialized(_this3), position);
    vars.reversed && _this3.reverse();
    vars.paused && _this3.paused(true);
    if (immediateRender || !duration && !keyframes && _this3._start === _roundPrecise(parent._time) && _isNotFalse(immediateRender) && _hasNoPausedAncestors(_assertThisInitialized(_this3)) && parent.data !== "nested") {
      _this3._tTime = -_tinyNum;
      _this3.render(Math.max(0, -delay) || 0);
    }
    scrollTrigger && _scrollTrigger(_assertThisInitialized(_this3), scrollTrigger);
    return _this3;
  }
  var _proto3 = Tween2.prototype;
  _proto3.render = function render14(totalTime, suppressEvents, force) {
    var prevTime = this._time, tDur = this._tDur, dur = this._dur, isNegative = totalTime < 0, tTime = totalTime > tDur - _tinyNum && !isNegative ? tDur : totalTime < _tinyNum ? 0 : totalTime, time, pt, iteration, cycleDuration, prevIteration, isYoyo, ratio, timeline2;
    if (!dur) {
      _renderZeroDurationTween(this, totalTime, suppressEvents, force);
    } else if (tTime !== this._tTime || !totalTime || force || !this._initted && this._tTime || this._startAt && this._zTime < 0 !== isNegative || this._lazy) {
      time = tTime;
      timeline2 = this.timeline;
      if (this._repeat) {
        cycleDuration = dur + this._rDelay;
        if (this._repeat < -1 && isNegative) {
          return this.totalTime(cycleDuration * 100 + totalTime, suppressEvents, force);
        }
        time = _roundPrecise(tTime % cycleDuration);
        if (tTime === tDur) {
          iteration = this._repeat;
          time = dur;
        } else {
          prevIteration = _roundPrecise(tTime / cycleDuration);
          iteration = ~~prevIteration;
          if (iteration && iteration === prevIteration) {
            time = dur;
            iteration--;
          } else if (time > dur) {
            time = dur;
          }
        }
        isYoyo = this._yoyo && iteration & 1;
        if (isYoyo) time = dur - time;
        prevIteration = _animationCycle(this._tTime, cycleDuration);
        if (time === prevTime && !force && this._initted && iteration === prevIteration) {
          this._tTime = tTime;
          return this;
        }
        if (iteration !== prevIteration) {
          if (this.vars.repeatRefresh && !isYoyo && !this._lock && time !== cycleDuration && this._initted) {
            this._lock = force = 1;
            this.render(_roundPrecise(cycleDuration * iteration), true).invalidate()._lock = 0;
          }
        }
      }
      if (!this._initted) {
        if (_attemptInitTween(this, isNegative ? totalTime : time, force, suppressEvents, tTime)) {
          this._tTime = 0;
          return this;
        }
        if (prevTime !== this._time && !(force && this.vars.repeatRefresh && iteration !== prevIteration)) {
          return this;
        }
        if (dur !== this._dur) {
          return this.render(totalTime, suppressEvents, force);
        }
      }
      if (this._rEase) {
        var inv = time < prevTime;
        if (inv !== this._inv) {
          var segDur = inv ? prevTime : dur - prevTime;
          this._inv = inv;
          if (this._from) this.ratio = 1 - this.ratio;
          this._invRatio = this.ratio;
          this._invTime = prevTime;
          this._invRecip = segDur ? (inv ? -1 : 1) / segDur : 0;
          this._invScale = inv ? -this.ratio : 1 - this.ratio;
          this._invEase = inv ? this._rEase : this._ease;
        }
        this.ratio = ratio = this._invRatio + this._invScale * this._invEase((time - this._invTime) * this._invRecip);
      } else {
        this.ratio = ratio = this._ease(time / dur);
      }
      if (this._from) this.ratio = ratio = 1 - ratio;
      this._tTime = tTime;
      this._time = time;
      if (!this._act && this._ts) {
        this._act = 1;
        this._lazy = 0;
      }
      if (!prevTime && tTime && !suppressEvents && !prevIteration) {
        _callback(this, "onStart");
        if (this._tTime !== tTime) {
          return this;
        }
      }
      pt = this._pt;
      while (pt) {
        pt.r(ratio, pt.d);
        pt = pt._next;
      }
      timeline2 && timeline2.render(totalTime < 0 ? totalTime : timeline2._dur * timeline2._ease(time / this._dur), suppressEvents, force) || this._startAt && (this._zTime = totalTime);
      if (this._onUpdate && !suppressEvents) {
        isNegative && _rewindStartAt(this, totalTime, suppressEvents, force);
        _callback(this, "onUpdate");
      }
      this._repeat && iteration !== prevIteration && this.vars.onRepeat && !suppressEvents && this.parent && _callback(this, "onRepeat");
      if ((tTime === this._tDur || !tTime) && this._tTime === tTime) {
        isNegative && !this._onUpdate && _rewindStartAt(this, totalTime, true, true);
        (totalTime || !dur) && (tTime === this._tDur && this._ts > 0 || !tTime && this._ts < 0) && _removeFromParent(this, 1);
        if (!suppressEvents && !(isNegative && !prevTime) && (tTime || prevTime || isYoyo)) {
          _callback(this, tTime === tDur ? "onComplete" : "onReverseComplete", true);
          this._prom && !(tTime < tDur && this.timeScale() > 0) && this._prom();
        }
      }
    }
    return this;
  };
  _proto3.targets = function targets() {
    return this._targets;
  };
  _proto3.invalidate = function invalidate(soft) {
    (!soft || !this.vars.runBackwards) && (this._startAt = 0);
    this._pt = this._op = this._onUpdate = this._lazy = this.ratio = 0;
    this._ptLookup = [];
    this.timeline && this.timeline.invalidate(soft);
    return _Animation2.prototype.invalidate.call(this, soft);
  };
  _proto3.resetTo = function resetTo(property, value, start, startIsRelative, skipRecursion) {
    _tickerActive || _ticker.wake();
    this._ts || this.play();
    var time = Math.min(this._dur, (this._dp._time - this._start) * this._ts), ratio;
    this._initted || _initTween(this, time);
    ratio = this._ease(time / this._dur);
    if (_updatePropTweens(this, property, value, start, startIsRelative, ratio, time, skipRecursion)) {
      return this.resetTo(property, value, start, startIsRelative, 1);
    }
    _alignPlayhead(this, 0);
    this.parent || _addLinkedListItem(this._dp, this, "_first", "_last", this._dp._sort ? "_start" : 0);
    return this.render(0);
  };
  _proto3.kill = function kill5(targets, vars) {
    if (vars === void 0) {
      vars = "all";
    }
    if (!targets && (!vars || vars === "all")) {
      this._lazy = this._pt = 0;
      this.parent ? _interrupt(this) : this.scrollTrigger && this.scrollTrigger.kill(!!_reverting);
      return this;
    }
    if (this.timeline) {
      var tDur = this.timeline.totalDuration();
      this.timeline.killTweensOf(targets, vars, _overwritingTween && _overwritingTween.vars.overwrite !== true)._first || _interrupt(this);
      this.parent && tDur !== this.timeline.totalDuration() && _setDuration(this, this._dur * this.timeline._tDur / tDur, 0, 1);
      return this;
    }
    var parsedTargets = this._targets, killingTargets = targets ? toArray(targets) : parsedTargets, propTweenLookup = this._ptLookup, firstPT = this._pt, overwrittenProps, curLookup, curOverwriteProps, props, p2, pt, i2;
    if ((!vars || vars === "all") && _arraysMatch(parsedTargets, killingTargets)) {
      vars === "all" && (this._pt = 0);
      return _interrupt(this);
    }
    overwrittenProps = this._op = this._op || [];
    if (vars !== "all") {
      if (_isString(vars)) {
        p2 = {};
        _forEachName(vars, function(name) {
          return p2[name] = 1;
        });
        vars = p2;
      }
      vars = _addAliasesToVars(parsedTargets, vars);
    }
    i2 = parsedTargets.length;
    while (i2--) {
      if (~killingTargets.indexOf(parsedTargets[i2])) {
        curLookup = propTweenLookup[i2];
        if (vars === "all") {
          overwrittenProps[i2] = vars;
          props = curLookup;
          curOverwriteProps = {};
        } else {
          curOverwriteProps = overwrittenProps[i2] = overwrittenProps[i2] || {};
          props = vars;
        }
        for (p2 in props) {
          pt = curLookup && curLookup[p2];
          if (pt) {
            if (!("kill" in pt.d) || pt.d.kill(p2) === true) {
              _removeLinkedListItem(this, pt, "_pt");
            }
            delete curLookup[p2];
          }
          if (curOverwriteProps !== "all") {
            curOverwriteProps[p2] = 1;
          }
        }
      }
    }
    this._initted && !this._pt && firstPT && _interrupt(this);
    return this;
  };
  Tween2.to = function to(targets, vars) {
    return new Tween2(targets, vars, arguments[2]);
  };
  Tween2.from = function from(targets, vars) {
    return _createTweenType(1, arguments);
  };
  Tween2.delayedCall = function delayedCall(delay, callback, params, scope) {
    return new Tween2(callback, 0, {
      immediateRender: false,
      lazy: false,
      overwrite: false,
      delay,
      onComplete: callback,
      onReverseComplete: callback,
      onCompleteParams: params,
      onReverseCompleteParams: params,
      callbackScope: scope
    });
  };
  Tween2.fromTo = function fromTo(targets, fromVars, toVars) {
    return _createTweenType(2, arguments);
  };
  Tween2.set = function set(targets, vars) {
    vars.duration = 0;
    vars.repeatDelay || (vars.repeat = 0);
    return new Tween2(targets, vars);
  };
  Tween2.killTweensOf = function killTweensOf(targets, props, onlyActive) {
    return _globalTimeline.killTweensOf(targets, props, onlyActive);
  };
  return Tween2;
})(Animation);
_setDefaults(Tween.prototype, {
  _targets: [],
  _lazy: 0,
  _startAt: 0,
  _op: 0,
  _onInit: 0
});
_forEachName("staggerTo,staggerFrom,staggerFromTo", function(name) {
  Tween[name] = function() {
    var tl = new Timeline(), params = _slice.call(arguments, 0);
    params.splice(name === "staggerFromTo" ? 5 : 4, 0, 0);
    return tl[name].apply(tl, params);
  };
});
var _setterPlain = function _setterPlain2(target, property, value) {
  return target[property] = value;
};
var _setterFunc = function _setterFunc2(target, property, value) {
  return target[property](value);
};
var _setterFuncWithParam = function _setterFuncWithParam2(target, property, value, data) {
  return target[property](data.fp, value);
};
var _setterAttribute = function _setterAttribute2(target, property, value) {
  return target.setAttribute(property, value);
};
var _getSetter = function _getSetter2(target, property) {
  return _isFunction(target[property]) ? _setterFunc : _isUndefined(target[property]) && target.setAttribute ? _setterAttribute : _setterPlain;
};
var _renderPlain = function _renderPlain2(ratio, data) {
  return data.set(data.t, data.p, Math.round((data.s + data.c * ratio) * 1e6) / 1e6, data);
};
var _renderBoolean = function _renderBoolean2(ratio, data) {
  return data.set(data.t, data.p, !!(data.s + data.c * ratio), data);
};
var _renderComplexString = function _renderComplexString2(ratio, data) {
  var pt = data._pt, s = "";
  if (!ratio && data.b) {
    s = data.b;
  } else if (ratio === 1 && data.e) {
    s = data.e;
  } else {
    while (pt) {
      s = pt.p + (pt.m ? pt.m(pt.s + pt.c * ratio) : Math.round((pt.s + pt.c * ratio) * 1e4) / 1e4) + s;
      pt = pt._next;
    }
    s += data.c;
  }
  data.set(data.t, data.p, s, data);
};
var _renderPropTweens = function _renderPropTweens2(ratio, data) {
  var pt = data._pt;
  while (pt) {
    pt.r(ratio, pt.d);
    pt = pt._next;
  }
};
var _addPluginModifier = function _addPluginModifier2(modifier, tween, target, property) {
  var pt = this._pt, next;
  while (pt) {
    next = pt._next;
    pt.p === property && pt.modifier(modifier, tween, target);
    pt = next;
  }
};
var _killPropTweensOf = function _killPropTweensOf2(property) {
  var pt = this._pt, hasNonDependentRemaining, next;
  while (pt) {
    next = pt._next;
    if (pt.p === property && !pt.op || pt.op === property) {
      _removeLinkedListItem(this, pt, "_pt");
    } else if (!pt.dep) {
      hasNonDependentRemaining = 1;
    }
    pt = next;
  }
  return !hasNonDependentRemaining;
};
var _setterWithModifier = function _setterWithModifier2(target, property, value, data) {
  data.mSet(target, property, data.m.call(data.tween, value, data.mt), data);
};
var _sortPropTweensByPriority = function _sortPropTweensByPriority2(parent) {
  var pt = parent._pt, next, pt2, first, last;
  while (pt) {
    next = pt._next;
    pt2 = first;
    while (pt2 && pt2.pr > pt.pr) {
      pt2 = pt2._next;
    }
    if (pt._prev = pt2 ? pt2._prev : last) {
      pt._prev._next = pt;
    } else {
      first = pt;
    }
    if (pt._next = pt2) {
      pt2._prev = pt;
    } else {
      last = pt;
    }
    pt = next;
  }
  parent._pt = first;
};
var PropTween = /* @__PURE__ */ (function() {
  function PropTween5(next, target, prop, start, change, renderer, data, setter, priority) {
    this.t = target;
    this.s = start;
    this.c = change;
    this.p = prop;
    this.r = renderer || _renderPlain;
    this.d = data || this;
    this.set = setter || _setterPlain;
    this.pr = priority || 0;
    this._next = next;
    if (next) {
      next._prev = this;
    }
  }
  var _proto4 = PropTween5.prototype;
  _proto4.modifier = function modifier(func, tween, target) {
    this.mSet = this.mSet || this.set;
    this.set = _setterWithModifier;
    this.m = func;
    this.mt = target;
    this.tween = tween;
  };
  return PropTween5;
})();
_forEachName(_callbackNames + "parent,duration,ease,delay,overwrite,runBackwards,startAt,yoyo,immediateRender,repeat,repeatDelay,data,paused,reversed,lazy,callbackScope,stringFilter,id,yoyoEase,stagger,inherit,repeatRefresh,keyframes,autoRevert,scrollTrigger,easeReverse", function(name) {
  return _reservedProps[name] = 1;
});
_globals.TweenMax = _globals.TweenLite = Tween;
_globals.TimelineLite = _globals.TimelineMax = Timeline;
_globalTimeline = new Timeline({
  sortChildren: false,
  defaults: _defaults,
  autoRemoveChildren: true,
  id: "root",
  smoothChildTiming: true
});
_config.stringFilter = _colorStringFilter;
var _media = [];
var _listeners = {};
var _emptyArray = [];
var _lastMediaTime = 0;
var _contextID = 0;
var _dispatch = function _dispatch2(type) {
  return (_listeners[type] || _emptyArray).map(function(f) {
    return f();
  });
};
var _onMediaChange = function _onMediaChange2() {
  var time = Date.now(), matches = [];
  if (time - _lastMediaTime > 2) {
    _dispatch("matchMediaInit");
    _media.forEach(function(c) {
      var queries = c.queries, conditions = c.conditions, match, p2, anyMatch, toggled;
      for (p2 in queries) {
        match = _win.matchMedia(queries[p2]).matches;
        match && (anyMatch = 1);
        if (match !== conditions[p2]) {
          conditions[p2] = match;
          toggled = 1;
        }
      }
      if (toggled) {
        c.revert();
        anyMatch && matches.push(c);
      }
    });
    _dispatch("matchMediaRevert");
    matches.forEach(function(c) {
      return c.onMatch(c, function(func) {
        return c.add(null, func);
      });
    });
    _lastMediaTime = time;
    _dispatch("matchMedia");
  }
};
var Context = /* @__PURE__ */ (function() {
  function Context2(func, scope) {
    this.selector = scope && selector(scope);
    this.data = [];
    this._r = [];
    this.isReverted = false;
    this.id = _contextID++;
    func && this.add(func);
  }
  var _proto5 = Context2.prototype;
  _proto5.add = function add(name, func, scope) {
    if (_isFunction(name)) {
      scope = func;
      func = name;
      name = _isFunction;
    }
    var self = this, f = function f2() {
      var prev = _context, prevSelector = self.selector, result;
      prev && prev !== self && prev.data.push(self);
      scope && (self.selector = selector(scope));
      _context = self;
      result = func.apply(self, arguments);
      _isFunction(result) && self._r.push(result);
      _context = prev;
      self.selector = prevSelector;
      self.isReverted = false;
      return result;
    };
    self.last = f;
    return name === _isFunction ? f(self, function(func2) {
      return self.add(null, func2);
    }) : name ? self[name] = f : f;
  };
  _proto5.ignore = function ignore(func) {
    var prev = _context;
    _context = null;
    func(this);
    _context = prev;
  };
  _proto5.getTweens = function getTweens() {
    var a = [];
    this.data.forEach(function(e) {
      return e instanceof Context2 ? a.push.apply(a, e.getTweens()) : e instanceof Tween && !(e.parent && e.parent.data === "nested") && a.push(e);
    });
    return a;
  };
  _proto5.clear = function clear() {
    this._r.length = this.data.length = 0;
  };
  _proto5.kill = function kill5(revert, matchMedia2) {
    var _this4 = this;
    if (revert) {
      (function() {
        var tweens = _this4.getTweens(), i3 = _this4.data.length, t;
        while (i3--) {
          t = _this4.data[i3];
          if (t.data === "isFlip") {
            t.revert();
            t.getChildren(true, true, false).forEach(function(tween) {
              return tweens.splice(tweens.indexOf(tween), 1);
            });
          }
        }
        tweens.map(function(t2) {
          return {
            g: t2._dur || t2._delay || t2._sat && !t2._sat.vars.immediateRender ? t2.globalTime(0) : -Infinity,
            t: t2
          };
        }).sort(function(a, b) {
          return b.g - a.g || -Infinity;
        }).forEach(function(o) {
          return o.t.revert(revert);
        });
        i3 = _this4.data.length;
        while (i3--) {
          t = _this4.data[i3];
          if (t instanceof Timeline) {
            if (t.data !== "nested") {
              t.scrollTrigger && t.scrollTrigger.revert();
              t.kill();
            }
          } else {
            !(t instanceof Tween) && t.revert && t.revert(revert);
          }
        }
        _this4._r.forEach(function(f) {
          return f(revert, _this4);
        });
        _this4.isReverted = true;
      })();
    } else {
      this.data.forEach(function(e) {
        return e.kill && e.kill();
      });
    }
    this.clear();
    if (matchMedia2) {
      var i2 = _media.length;
      while (i2--) {
        _media[i2].id === this.id && _media.splice(i2, 1);
      }
    }
  };
  _proto5.revert = function revert(config3) {
    this.kill(config3 || {});
  };
  return Context2;
})();
var MatchMedia = /* @__PURE__ */ (function() {
  function MatchMedia2(scope) {
    this.contexts = [];
    this.scope = scope;
    _context && _context.data.push(this);
  }
  var _proto6 = MatchMedia2.prototype;
  _proto6.add = function add(conditions, func, scope) {
    _isObject(conditions) || (conditions = {
      matches: conditions
    });
    var context3 = new Context(0, scope || this.scope), cond = context3.conditions = {}, mq, p2, active;
    _context && !context3.selector && (context3.selector = _context.selector);
    this.contexts.push(context3);
    func = context3.add("onMatch", func);
    context3.queries = conditions;
    for (p2 in conditions) {
      if (p2 === "all") {
        active = 1;
      } else {
        mq = _win.matchMedia(conditions[p2]);
        if (mq) {
          _media.indexOf(context3) < 0 && _media.push(context3);
          (cond[p2] = mq.matches) && (active = 1);
          mq.addListener ? mq.addListener(_onMediaChange) : mq.addEventListener("change", _onMediaChange);
        }
      }
    }
    active && func(context3, function(f) {
      return context3.add(null, f);
    });
    return this;
  };
  _proto6.revert = function revert(config3) {
    this.kill(config3 || {});
  };
  _proto6.kill = function kill5(revert) {
    this.contexts.forEach(function(c) {
      return c.kill(revert, true);
    });
  };
  return MatchMedia2;
})();
var _gsap = {
  registerPlugin: function registerPlugin() {
    for (var _len2 = arguments.length, args = new Array(_len2), _key2 = 0; _key2 < _len2; _key2++) {
      args[_key2] = arguments[_key2];
    }
    args.forEach(function(config3) {
      return _createPlugin(config3);
    });
  },
  timeline: function timeline(vars) {
    return new Timeline(vars);
  },
  getTweensOf: function getTweensOf(targets, onlyActive) {
    return _globalTimeline.getTweensOf(targets, onlyActive);
  },
  getProperty: function getProperty(target, property, unit, uncache) {
    _isString(target) && (target = toArray(target)[0]);
    var getter = _getCache(target || {}).get, format = unit ? _passThrough : _numericIfPossible;
    unit === "native" && (unit = "");
    return !target ? target : !property ? function(property2, unit2, uncache2) {
      return format((_plugins[property2] && _plugins[property2].get || getter)(target, property2, unit2, uncache2));
    } : format((_plugins[property] && _plugins[property].get || getter)(target, property, unit, uncache));
  },
  quickSetter: function quickSetter(target, property, unit) {
    target = toArray(target);
    if (target.length > 1) {
      var setters = target.map(function(t) {
        return gsap.quickSetter(t, property, unit);
      }), l = setters.length;
      return function(value) {
        var i2 = l;
        while (i2--) {
          setters[i2](value);
        }
      };
    }
    target = target[0] || {};
    var Plugin = _plugins[property], cache = _getCache(target), p2 = cache.harness && (cache.harness.aliases || {})[property] || property, setter = Plugin ? function(value) {
      var p3 = new Plugin();
      _quickTween._pt = 0;
      p3.init(target, unit ? value + unit : value, _quickTween, 0, [target]);
      p3.render(1, p3);
      _quickTween._pt && _renderPropTweens(1, _quickTween);
    } : cache.set(target, p2);
    return Plugin ? setter : function(value) {
      return setter(target, p2, unit ? value + unit : value, cache, 1);
    };
  },
  quickTo: function quickTo(target, property, vars) {
    var _setDefaults22;
    var tween = gsap.to(target, _setDefaults((_setDefaults22 = {}, _setDefaults22[property] = "+=0.1", _setDefaults22.paused = true, _setDefaults22.stagger = 0, _setDefaults22), vars || {})), func = function func2(value, start, startIsRelative) {
      return tween.resetTo(property, value, start, startIsRelative);
    };
    func.tween = tween;
    return func;
  },
  isTweening: function isTweening(targets) {
    return _globalTimeline.getTweensOf(targets, true).length > 0;
  },
  defaults: function defaults(value) {
    value && value.ease && (value.ease = _parseEase(value.ease, _defaults.ease));
    return _mergeDeep(_defaults, value || {});
  },
  config: function config2(value) {
    return _mergeDeep(_config, value || {});
  },
  registerEffect: function registerEffect(_ref3) {
    var name = _ref3.name, effect = _ref3.effect, plugins = _ref3.plugins, defaults2 = _ref3.defaults, extendTimeline = _ref3.extendTimeline;
    (plugins || "").split(",").forEach(function(pluginName) {
      return pluginName && !_plugins[pluginName] && !_globals[pluginName] && _warn(name + " effect requires " + pluginName + " plugin.");
    });
    _effects[name] = function(targets, vars, tl) {
      return effect(toArray(targets), _setDefaults(vars || {}, defaults2), tl);
    };
    if (extendTimeline) {
      Timeline.prototype[name] = function(targets, vars, position) {
        return this.add(_effects[name](targets, _isObject(vars) ? vars : (position = vars) && {}, this), position);
      };
    }
  },
  registerEase: function registerEase(name, ease) {
    _easeMap[name] = _parseEase(ease);
  },
  parseEase: function parseEase(ease, defaultEase) {
    return arguments.length ? _parseEase(ease, defaultEase) : _easeMap;
  },
  getById: function getById(id) {
    return _globalTimeline.getById(id);
  },
  exportRoot: function exportRoot(vars, includeDelayedCalls) {
    if (vars === void 0) {
      vars = {};
    }
    var tl = new Timeline(vars), child, next;
    tl.smoothChildTiming = _isNotFalse(vars.smoothChildTiming);
    _globalTimeline.remove(tl);
    tl._dp = 0;
    tl._time = tl._tTime = _globalTimeline._time;
    child = _globalTimeline._first;
    while (child) {
      next = child._next;
      if (includeDelayedCalls || !(!child._dur && child instanceof Tween && child.vars.onComplete === child._targets[0])) {
        _addToTimeline(tl, child, child._start - child._delay);
      }
      child = next;
    }
    _addToTimeline(_globalTimeline, tl, 0);
    return tl;
  },
  context: function context(func, scope) {
    return func ? new Context(func, scope) : _context;
  },
  matchMedia: function matchMedia(scope) {
    return new MatchMedia(scope);
  },
  matchMediaRefresh: function matchMediaRefresh() {
    return _media.forEach(function(c) {
      var cond = c.conditions, found, p2;
      for (p2 in cond) {
        if (cond[p2]) {
          cond[p2] = false;
          found = 1;
        }
      }
      found && c.revert();
    }) || _onMediaChange();
  },
  addEventListener: function addEventListener(type, callback) {
    var a = _listeners[type] || (_listeners[type] = []);
    ~a.indexOf(callback) || a.push(callback);
  },
  removeEventListener: function removeEventListener(type, callback) {
    var a = _listeners[type], i2 = a && a.indexOf(callback);
    i2 >= 0 && a.splice(i2, 1);
  },
  utils: {
    wrap,
    wrapYoyo,
    distribute,
    random,
    snap,
    normalize,
    getUnit,
    clamp,
    splitColor,
    toArray,
    selector,
    mapRange,
    pipe,
    unitize,
    interpolate,
    shuffle
  },
  install: _install,
  effects: _effects,
  ticker: _ticker,
  updateRoot: Timeline.updateRoot,
  plugins: _plugins,
  globalTimeline: _globalTimeline,
  core: {
    PropTween,
    globals: _addGlobal,
    Tween,
    Timeline,
    Animation,
    getCache: _getCache,
    _removeLinkedListItem,
    reverting: function reverting() {
      return _reverting;
    },
    context: function context2(toAdd) {
      if (toAdd && _context) {
        _context.data.push(toAdd);
        toAdd._ctx = _context;
      }
      return _context;
    },
    suppressOverwrites: function suppressOverwrites(value) {
      return _suppressOverwrites = value;
    }
  }
};
_forEachName("to,from,fromTo,delayedCall,set,killTweensOf", function(name) {
  return _gsap[name] = Tween[name];
});
_ticker.add(Timeline.updateRoot);
_quickTween = _gsap.to({}, {
  duration: 0
});
var _getPluginPropTween = function _getPluginPropTween2(plugin, prop) {
  var pt = plugin._pt;
  while (pt && pt.p !== prop && pt.op !== prop && pt.fp !== prop) {
    pt = pt._next;
  }
  return pt;
};
var _addModifiers = function _addModifiers2(tween, modifiers) {
  var targets = tween._targets, p2, i2, pt;
  for (p2 in modifiers) {
    i2 = targets.length;
    while (i2--) {
      pt = tween._ptLookup[i2][p2];
      if (pt && (pt = pt.d)) {
        if (pt._pt) {
          pt = _getPluginPropTween(pt, p2);
        }
        pt && pt.modifier && pt.modifier(modifiers[p2], tween, targets[i2], p2);
      }
    }
  }
};
var _buildModifierPlugin = function _buildModifierPlugin2(name, modifier) {
  return {
    name,
    headless: 1,
    rawVars: 1,
    //don't pre-process function-based values or "random()" strings.
    init: function init16(target, vars, tween) {
      tween._onInit = function(tween2) {
        var temp, p2;
        if (_isString(vars)) {
          temp = {};
          _forEachName(vars, function(name2) {
            return temp[name2] = 1;
          });
          vars = temp;
        }
        if (modifier) {
          temp = {};
          for (p2 in vars) {
            temp[p2] = modifier(vars[p2]);
          }
          vars = temp;
        }
        _addModifiers(tween2, vars);
      };
    }
  };
};
var gsap = _gsap.registerPlugin({
  name: "attr",
  init: function init(target, vars, tween, index, targets) {
    var p2, pt, v;
    this.tween = tween;
    for (p2 in vars) {
      v = target.getAttribute(p2) || "";
      pt = this.add(target, "setAttribute", (v || 0) + "", vars[p2], index, targets, 0, 0, p2);
      pt.op = p2;
      pt.b = v;
      this._props.push(p2);
    }
  },
  render: function render(ratio, data) {
    var pt = data._pt;
    while (pt) {
      _reverting ? pt.set(pt.t, pt.p, pt.b, pt) : pt.r(ratio, pt.d);
      pt = pt._next;
    }
  }
}, {
  name: "endArray",
  headless: 1,
  init: function init2(target, value) {
    var i2 = value.length;
    while (i2--) {
      this.add(target, i2, target[i2] || 0, value[i2], 0, 0, 0, 0, 0, 1);
    }
  }
}, _buildModifierPlugin("roundProps", _roundModifier), _buildModifierPlugin("modifiers"), _buildModifierPlugin("snap", snap)) || _gsap;
Tween.version = Timeline.version = gsap.version = "3.15.0";
_coreReady = 1;
_windowExists() && _wake();
var Power0 = _easeMap.Power0;
var Power1 = _easeMap.Power1;
var Power2 = _easeMap.Power2;
var Power3 = _easeMap.Power3;
var Power4 = _easeMap.Power4;
var Linear = _easeMap.Linear;
var Quad = _easeMap.Quad;
var Cubic = _easeMap.Cubic;
var Quart = _easeMap.Quart;
var Quint = _easeMap.Quint;
var Strong = _easeMap.Strong;
var Elastic = _easeMap.Elastic;
var Back = _easeMap.Back;
var SteppedEase = _easeMap.SteppedEase;
var Bounce = _easeMap.Bounce;
var Sine = _easeMap.Sine;
var Expo = _easeMap.Expo;
var Circ = _easeMap.Circ;

// examples/figma-launch/node_modules/gsap/CSSPlugin.js
var _win2;
var _doc2;
var _docElement;
var _pluginInitted;
var _tempDiv;
var _tempDivStyler;
var _recentSetterPlugin;
var _reverting2;
var _windowExists3 = function _windowExists4() {
  return typeof window !== "undefined";
};
var _transformProps = {};
var _RAD2DEG = 180 / Math.PI;
var _DEG2RAD = Math.PI / 180;
var _atan2 = Math.atan2;
var _bigNum2 = 1e8;
var _capsExp = /([A-Z])/g;
var _horizontalExp = /(left|right|width|margin|padding|x)/i;
var _complexExp = /[\s,\(]\S/;
var _propertyAliases = {
  autoAlpha: "opacity,visibility",
  scale: "scaleX,scaleY",
  alpha: "opacity"
};
var _renderCSSProp = function _renderCSSProp2(ratio, data) {
  return data.set(data.t, data.p, Math.round((data.s + data.c * ratio) * 1e4) / 1e4 + data.u, data);
};
var _renderPropWithEnd = function _renderPropWithEnd2(ratio, data) {
  return data.set(data.t, data.p, ratio === 1 ? data.e : Math.round((data.s + data.c * ratio) * 1e4) / 1e4 + data.u, data);
};
var _renderCSSPropWithBeginning = function _renderCSSPropWithBeginning2(ratio, data) {
  return data.set(data.t, data.p, ratio ? Math.round((data.s + data.c * ratio) * 1e4) / 1e4 + data.u : data.b, data);
};
var _renderCSSPropWithBeginningAndEnd = function _renderCSSPropWithBeginningAndEnd2(ratio, data) {
  return data.set(data.t, data.p, ratio === 1 ? data.e : ratio ? Math.round((data.s + data.c * ratio) * 1e4) / 1e4 + data.u : data.b, data);
};
var _renderRoundedCSSProp = function _renderRoundedCSSProp2(ratio, data) {
  var value = data.s + data.c * ratio;
  data.set(data.t, data.p, ~~(value + (value < 0 ? -0.5 : 0.5)) + data.u, data);
};
var _renderNonTweeningValue = function _renderNonTweeningValue2(ratio, data) {
  return data.set(data.t, data.p, ratio ? data.e : data.b, data);
};
var _renderNonTweeningValueOnlyAtEnd = function _renderNonTweeningValueOnlyAtEnd2(ratio, data) {
  return data.set(data.t, data.p, ratio !== 1 ? data.b : data.e, data);
};
var _setterCSSStyle = function _setterCSSStyle2(target, property, value) {
  return target.style[property] = value;
};
var _setterCSSProp = function _setterCSSProp2(target, property, value) {
  return target.style.setProperty(property, value);
};
var _setterTransform = function _setterTransform2(target, property, value) {
  return target._gsap[property] = value;
};
var _setterScale = function _setterScale2(target, property, value) {
  return target._gsap.scaleX = target._gsap.scaleY = value;
};
var _setterScaleWithRender = function _setterScaleWithRender2(target, property, value, data, ratio) {
  var cache = target._gsap;
  cache.scaleX = cache.scaleY = value;
  cache.renderTransform(ratio, cache);
};
var _setterTransformWithRender = function _setterTransformWithRender2(target, property, value, data, ratio) {
  var cache = target._gsap;
  cache[property] = value;
  cache.renderTransform(ratio, cache);
};
var _transformProp = "transform";
var _transformOriginProp = _transformProp + "Origin";
var _saveStyle = function _saveStyle2(property, isNotCSS) {
  var _this = this;
  var target = this.target, style = target.style, cache = target._gsap;
  if (property in _transformProps && style) {
    this.tfm = this.tfm || {};
    if (property !== "transform") {
      property = _propertyAliases[property] || property;
      ~property.indexOf(",") ? property.split(",").forEach(function(a) {
        return _this.tfm[a] = _get(target, a);
      }) : this.tfm[property] = cache.x ? cache[property] : _get(target, property);
      property === _transformOriginProp && (this.tfm.zOrigin = cache.zOrigin);
    } else {
      return _propertyAliases.transform.split(",").forEach(function(p2) {
        return _saveStyle2.call(_this, p2, isNotCSS);
      });
    }
    if (this.props.indexOf(_transformProp) >= 0) {
      return;
    }
    if (cache.svg) {
      this.svgo = target.getAttribute("data-svg-origin");
      this.props.push(_transformOriginProp, isNotCSS, "");
    }
    property = _transformProp;
  }
  (style || isNotCSS) && this.props.push(property, isNotCSS, style[property]);
};
var _removeIndependentTransforms = function _removeIndependentTransforms2(style) {
  if (style.translate) {
    style.removeProperty("translate");
    style.removeProperty("scale");
    style.removeProperty("rotate");
  }
};
var _revertStyle = function _revertStyle2() {
  var props = this.props, target = this.target, style = target.style, cache = target._gsap, i2, p2;
  for (i2 = 0; i2 < props.length; i2 += 3) {
    if (!props[i2 + 1]) {
      props[i2 + 2] ? style[props[i2]] = props[i2 + 2] : style.removeProperty(props[i2].substr(0, 2) === "--" ? props[i2] : props[i2].replace(_capsExp, "-$1").toLowerCase());
    } else if (props[i2 + 1] === 2) {
      target[props[i2]](props[i2 + 2]);
    } else {
      target[props[i2]] = props[i2 + 2];
    }
  }
  if (this.tfm) {
    for (p2 in this.tfm) {
      cache[p2] = this.tfm[p2];
    }
    if (cache.svg) {
      cache.renderTransform();
      target.setAttribute("data-svg-origin", this.svgo || "");
    }
    i2 = _reverting2();
    if ((!i2 || !i2.isStart) && !style[_transformProp]) {
      _removeIndependentTransforms(style);
      if (cache.zOrigin && style[_transformOriginProp]) {
        style[_transformOriginProp] += " " + cache.zOrigin + "px";
        cache.zOrigin = 0;
        cache.renderTransform();
      }
      cache.uncache = 1;
    }
  }
};
var _getStyleSaver = function _getStyleSaver2(target, properties) {
  var saver = {
    target,
    props: [],
    revert: _revertStyle,
    save: _saveStyle
  };
  target._gsap || gsap.core.getCache(target);
  properties && target.style && target.nodeType && properties.split(",").forEach(function(p2) {
    return saver.save(p2);
  });
  return saver;
};
var _supports3D;
var _createElement = function _createElement2(type, ns) {
  var e = _doc2.createElementNS ? _doc2.createElementNS((ns || "http://www.w3.org/1999/xhtml").replace(/^https/, "http"), type) : _doc2.createElement(type);
  return e && e.style ? e : _doc2.createElement(type);
};
var _getComputedProperty = function _getComputedProperty2(target, property, skipPrefixFallback) {
  var cs = getComputedStyle(target);
  return cs[property] || cs.getPropertyValue(property.replace(_capsExp, "-$1").toLowerCase()) || cs.getPropertyValue(property) || !skipPrefixFallback && _getComputedProperty2(target, _checkPropPrefix(property) || property, 1) || "";
};
var _prefixes = "O,Moz,ms,Ms,Webkit".split(",");
var _checkPropPrefix = function _checkPropPrefix2(property, element, preferPrefix) {
  var e = element || _tempDiv, s = e.style, i2 = 5;
  if (property in s && !preferPrefix) {
    return property;
  }
  property = property.charAt(0).toUpperCase() + property.substr(1);
  while (i2-- && !(_prefixes[i2] + property in s)) {
  }
  return i2 < 0 ? null : (i2 === 3 ? "ms" : i2 >= 0 ? _prefixes[i2] : "") + property;
};
var _initCore = function _initCore2() {
  if (_windowExists3() && window.document) {
    _win2 = window;
    _doc2 = _win2.document;
    _docElement = _doc2.documentElement;
    _tempDiv = _createElement("div") || {
      style: {}
    };
    _tempDivStyler = _createElement("div");
    _transformProp = _checkPropPrefix(_transformProp);
    _transformOriginProp = _transformProp + "Origin";
    _tempDiv.style.cssText = "border-width:0;line-height:0;position:absolute;padding:0";
    _supports3D = !!_checkPropPrefix("perspective");
    _reverting2 = gsap.core.reverting;
    _pluginInitted = 1;
  }
};
var _getReparentedCloneBBox = function _getReparentedCloneBBox2(target) {
  var owner = target.ownerSVGElement, svg = _createElement("svg", owner && owner.getAttribute("xmlns") || "http://www.w3.org/2000/svg"), clone = target.cloneNode(true), bbox;
  clone.style.display = "block";
  svg.appendChild(clone);
  _docElement.appendChild(svg);
  try {
    bbox = clone.getBBox();
  } catch (e) {
  }
  svg.removeChild(clone);
  _docElement.removeChild(svg);
  return bbox;
};
var _getAttributeFallbacks = function _getAttributeFallbacks2(target, attributesArray) {
  var i2 = attributesArray.length;
  while (i2--) {
    if (target.hasAttribute(attributesArray[i2])) {
      return target.getAttribute(attributesArray[i2]);
    }
  }
};
var _getBBox = function _getBBox2(target) {
  var bounds, cloned;
  try {
    bounds = target.getBBox();
  } catch (error) {
    bounds = _getReparentedCloneBBox(target);
    cloned = 1;
  }
  bounds && (bounds.width || bounds.height) || cloned || (bounds = _getReparentedCloneBBox(target));
  return bounds && !bounds.width && !bounds.x && !bounds.y ? {
    x: +_getAttributeFallbacks(target, ["x", "cx", "x1"]) || 0,
    y: +_getAttributeFallbacks(target, ["y", "cy", "y1"]) || 0,
    width: 0,
    height: 0
  } : bounds;
};
var _isSVG = function _isSVG2(e) {
  return !!(e.getCTM && (!e.parentNode || e.ownerSVGElement) && _getBBox(e));
};
var _removeProperty = function _removeProperty2(target, property) {
  if (property) {
    var style = target.style, first2Chars;
    if (property in _transformProps && property !== _transformOriginProp) {
      property = _transformProp;
    }
    if (style.removeProperty) {
      first2Chars = property.substr(0, 2);
      if (first2Chars === "ms" || property.substr(0, 6) === "webkit") {
        property = "-" + property;
      }
      style.removeProperty(first2Chars === "--" ? property : property.replace(_capsExp, "-$1").toLowerCase());
    } else {
      style.removeAttribute(property);
    }
  }
};
var _addNonTweeningPT = function _addNonTweeningPT2(plugin, target, property, beginning, end, onlySetAtEnd) {
  var pt = new PropTween(plugin._pt, target, property, 0, 1, onlySetAtEnd ? _renderNonTweeningValueOnlyAtEnd : _renderNonTweeningValue);
  plugin._pt = pt;
  pt.b = beginning;
  pt.e = end;
  plugin._props.push(property);
  return pt;
};
var _nonConvertibleUnits = {
  deg: 1,
  rad: 1,
  turn: 1
};
var _nonStandardLayouts = {
  grid: 1,
  flex: 1
};
var _convertToUnit = function _convertToUnit2(target, property, value, unit) {
  var curValue = parseFloat(value) || 0, curUnit = (value + "").trim().substr((curValue + "").length) || "px", style = _tempDiv.style, horizontal = _horizontalExp.test(property), isRootSVG = target.tagName.toLowerCase() === "svg", measureProperty = (isRootSVG ? "client" : "offset") + (horizontal ? "Width" : "Height"), amount = 100, toPixels = unit === "px", toPercent = unit === "%", px, parent, cache, isSVG;
  if (unit === curUnit || !curValue || _nonConvertibleUnits[unit] || _nonConvertibleUnits[curUnit]) {
    return curValue;
  }
  curUnit !== "px" && !toPixels && (curValue = _convertToUnit2(target, property, value, "px"));
  isSVG = target.getCTM && _isSVG(target);
  if ((toPercent || curUnit === "%") && (_transformProps[property] || ~property.indexOf("adius"))) {
    px = isSVG ? target.getBBox()[horizontal ? "width" : "height"] : target[measureProperty];
    return _round(toPercent ? curValue / px * amount : curValue / 100 * px);
  }
  style[horizontal ? "width" : "height"] = amount + (toPixels ? curUnit : unit);
  parent = unit !== "rem" && ~property.indexOf("adius") || unit === "em" && target.appendChild && !isRootSVG ? target : target.parentNode;
  if (isSVG) {
    parent = (target.ownerSVGElement || {}).parentNode;
  }
  if (!parent || parent === _doc2 || !parent.appendChild) {
    parent = _doc2.body;
  }
  cache = parent._gsap;
  if (cache && toPercent && cache.width && horizontal && cache.time === _ticker.time && !cache.uncache) {
    return _round(curValue / cache.width * amount);
  } else {
    if (toPercent && (property === "height" || property === "width")) {
      var v = target.style[property];
      target.style[property] = amount + unit;
      px = target[measureProperty];
      v ? target.style[property] = v : _removeProperty(target, property);
    } else {
      (toPercent || curUnit === "%") && !_nonStandardLayouts[_getComputedProperty(parent, "display")] && (style.position = _getComputedProperty(target, "position"));
      parent === target && (style.position = "static");
      parent.appendChild(_tempDiv);
      px = _tempDiv[measureProperty];
      parent.removeChild(_tempDiv);
      style.position = "absolute";
    }
    if (horizontal && toPercent) {
      cache = _getCache(parent);
      cache.time = _ticker.time;
      cache.width = parent[measureProperty];
    }
  }
  return _round(toPixels ? px * curValue / amount : px && curValue ? amount / px * curValue : 0);
};
var _get = function _get2(target, property, unit, uncache) {
  var value;
  _pluginInitted || _initCore();
  if (property in _propertyAliases && property !== "transform") {
    property = _propertyAliases[property];
    if (~property.indexOf(",")) {
      property = property.split(",")[0];
    }
  }
  if (_transformProps[property] && property !== "transform") {
    value = _parseTransform(target, uncache);
    value = property !== "transformOrigin" ? value[property] : value.svg ? value.origin : _firstTwoOnly(_getComputedProperty(target, _transformOriginProp)) + " " + value.zOrigin + "px";
  } else {
    value = target.style[property];
    if (!value || value === "auto" || uncache || ~(value + "").indexOf("calc(")) {
      value = _specialProps[property] && _specialProps[property](target, property, unit) || _getComputedProperty(target, property) || _getProperty(target, property) || (property === "opacity" ? 1 : 0);
    }
  }
  return unit && !~(value + "").trim().indexOf(" ") ? _convertToUnit(target, property, value, unit) + unit : value;
};
var _tweenComplexCSSString = function _tweenComplexCSSString2(target, prop, start, end) {
  if (!start || start === "none") {
    var p2 = _checkPropPrefix(prop, target, 1), s = p2 && _getComputedProperty(target, p2, 1);
    if (s && s !== start) {
      prop = p2;
      start = s;
    } else if (prop === "borderColor") {
      start = _getComputedProperty(target, "borderTopColor");
    }
  }
  var pt = new PropTween(this._pt, target.style, prop, 0, 1, _renderComplexString), index = 0, matchIndex = 0, a, result, startValues, startNum, color, startValue, endValue, endNum, chunk, endUnit, startUnit, endValues;
  pt.b = start;
  pt.e = end;
  start += "";
  end += "";
  if (end.substring(0, 6) === "var(--") {
    end = _getComputedProperty(target, end.substring(4, end.indexOf(")")));
  }
  if (end === "auto") {
    startValue = target.style[prop];
    target.style[prop] = end;
    end = _getComputedProperty(target, prop) || end;
    startValue ? target.style[prop] = startValue : _removeProperty(target, prop);
  }
  a = [start, end];
  _colorStringFilter(a);
  start = a[0];
  end = a[1];
  startValues = start.match(_numWithUnitExp) || [];
  endValues = end.match(_numWithUnitExp) || [];
  if (endValues.length) {
    while (result = _numWithUnitExp.exec(end)) {
      endValue = result[0];
      chunk = end.substring(index, result.index);
      if (color) {
        color = (color + 1) % 5;
      } else if (chunk.substr(-5) === "rgba(" || chunk.substr(-5) === "hsla(") {
        color = 1;
      }
      if (endValue !== (startValue = startValues[matchIndex++] || "")) {
        startNum = parseFloat(startValue) || 0;
        startUnit = startValue.substr((startNum + "").length);
        endValue.charAt(1) === "=" && (endValue = _parseRelative(startNum, endValue) + startUnit);
        endNum = parseFloat(endValue);
        endUnit = endValue.substr((endNum + "").length);
        index = _numWithUnitExp.lastIndex - endUnit.length;
        if (!endUnit) {
          endUnit = endUnit || _config.units[prop] || startUnit;
          if (index === end.length) {
            end += endUnit;
            pt.e += endUnit;
          }
        }
        if (startUnit !== endUnit) {
          startNum = _convertToUnit(target, prop, startValue, endUnit) || 0;
        }
        pt._pt = {
          _next: pt._pt,
          p: chunk || matchIndex === 1 ? chunk : ",",
          //note: SVG spec allows omission of comma/space when a negative sign is wedged between two numbers, like 2.5-5.3 instead of 2.5,-5.3 but when tweening, the negative value may switch to positive, so we insert the comma just in case.
          s: startNum,
          c: endNum - startNum,
          m: color && color < 4 || prop === "zIndex" ? Math.round : 0
        };
      }
    }
    pt.c = index < end.length ? end.substring(index, end.length) : "";
  } else {
    pt.r = prop === "display" && end === "none" ? _renderNonTweeningValueOnlyAtEnd : _renderNonTweeningValue;
  }
  _relExp.test(end) && (pt.e = 0);
  this._pt = pt;
  return pt;
};
var _keywordToPercent = {
  top: "0%",
  bottom: "100%",
  left: "0%",
  right: "100%",
  center: "50%"
};
var _convertKeywordsToPercentages = function _convertKeywordsToPercentages2(value) {
  var split = value.split(" "), x = split[0], y = split[1] || "50%";
  if (x === "top" || x === "bottom" || y === "left" || y === "right") {
    value = x;
    x = y;
    y = value;
  }
  split[0] = _keywordToPercent[x] || x;
  split[1] = _keywordToPercent[y] || y;
  return split.join(" ");
};
var _renderClearProps = function _renderClearProps2(ratio, data) {
  if (data.tween && data.tween._time === data.tween._dur) {
    var target = data.t, style = target.style, props = data.u, cache = target._gsap, prop, clearTransforms, i2;
    if (props === "all" || props === true) {
      style.cssText = "";
      clearTransforms = 1;
    } else {
      props = props.split(",");
      i2 = props.length;
      while (--i2 > -1) {
        prop = props[i2];
        if (_transformProps[prop]) {
          clearTransforms = 1;
          prop = prop === "transformOrigin" ? _transformOriginProp : _transformProp;
        }
        _removeProperty(target, prop);
      }
    }
    if (clearTransforms) {
      _removeProperty(target, _transformProp);
      if (cache) {
        cache.svg && target.removeAttribute("transform");
        style.scale = style.rotate = style.translate = "none";
        _parseTransform(target, 1);
        cache.uncache = 1;
        _removeIndependentTransforms(style);
      }
    }
  }
};
var _specialProps = {
  clearProps: function clearProps(plugin, target, property, endValue, tween) {
    if (tween.data !== "isFromStart") {
      var pt = plugin._pt = new PropTween(plugin._pt, target, property, 0, 0, _renderClearProps);
      pt.u = endValue;
      pt.pr = -10;
      pt.tween = tween;
      plugin._props.push(property);
      return 1;
    }
  }
  /* className feature (about 0.4kb gzipped).
  , className(plugin, target, property, endValue, tween) {
  	let _renderClassName = (ratio, data) => {
  			data.css.render(ratio, data.css);
  			if (!ratio || ratio === 1) {
  				let inline = data.rmv,
  					target = data.t,
  					p;
  				target.setAttribute("class", ratio ? data.e : data.b);
  				for (p in inline) {
  					_removeProperty(target, p);
  				}
  			}
  		},
  		_getAllStyles = (target) => {
  			let styles = {},
  				computed = getComputedStyle(target),
  				p;
  			for (p in computed) {
  				if (isNaN(p) && p !== "cssText" && p !== "length") {
  					styles[p] = computed[p];
  				}
  			}
  			_setDefaults(styles, _parseTransform(target, 1));
  			return styles;
  		},
  		startClassList = target.getAttribute("class"),
  		style = target.style,
  		cssText = style.cssText,
  		cache = target._gsap,
  		classPT = cache.classPT,
  		inlineToRemoveAtEnd = {},
  		data = {t:target, plugin:plugin, rmv:inlineToRemoveAtEnd, b:startClassList, e:(endValue.charAt(1) !== "=") ? endValue : startClassList.replace(new RegExp("(?:\\s|^)" + endValue.substr(2) + "(?![\\w-])"), "") + ((endValue.charAt(0) === "+") ? " " + endValue.substr(2) : "")},
  		changingVars = {},
  		startVars = _getAllStyles(target),
  		transformRelated = /(transform|perspective)/i,
  		endVars, p;
  	if (classPT) {
  		classPT.r(1, classPT.d);
  		_removeLinkedListItem(classPT.d.plugin, classPT, "_pt");
  	}
  	target.setAttribute("class", data.e);
  	endVars = _getAllStyles(target, true);
  	target.setAttribute("class", startClassList);
  	for (p in endVars) {
  		if (endVars[p] !== startVars[p] && !transformRelated.test(p)) {
  			changingVars[p] = endVars[p];
  			if (!style[p] && style[p] !== "0") {
  				inlineToRemoveAtEnd[p] = 1;
  			}
  		}
  	}
  	cache.classPT = plugin._pt = new PropTween(plugin._pt, target, "className", 0, 0, _renderClassName, data, 0, -11);
  	if (style.cssText !== cssText) { //only apply if things change. Otherwise, in cases like a background-image that's pulled dynamically, it could cause a refresh. See https://gsap.com/forums/topic/20368-possible-gsap-bug-switching-classnames-in-chrome/.
  		style.cssText = cssText; //we recorded cssText before we swapped classes and ran _getAllStyles() because in cases when a className tween is overwritten, we remove all the related tweening properties from that class change (otherwise class-specific stuff can't override properties we've directly set on the target's style object due to specificity).
  	}
  	_parseTransform(target, true); //to clear the caching of transforms
  	data.css = new gsap.plugins.css();
  	data.css.init(target, changingVars, tween);
  	plugin._props.push(...data.css._props);
  	return 1;
  }
  */
};
var _identity2DMatrix = [1, 0, 0, 1, 0, 0];
var _rotationalProperties = {};
var _isNullTransform = function _isNullTransform2(value) {
  return value === "matrix(1, 0, 0, 1, 0, 0)" || value === "none" || !value;
};
var _getComputedTransformMatrixAsArray = function _getComputedTransformMatrixAsArray2(target) {
  var matrixString = _getComputedProperty(target, _transformProp);
  return _isNullTransform(matrixString) ? _identity2DMatrix : matrixString.substr(7).match(_numExp).map(_round);
};
var _getMatrix = function _getMatrix2(target, force2D) {
  var cache = target._gsap || _getCache(target), style = target.style, matrix = _getComputedTransformMatrixAsArray(target), parent, nextSibling, temp, addedToDOM;
  if (cache.svg && target.getAttribute("transform")) {
    temp = target.transform.baseVal.consolidate().matrix;
    matrix = [temp.a, temp.b, temp.c, temp.d, temp.e, temp.f];
    return matrix.join(",") === "1,0,0,1,0,0" ? _identity2DMatrix : matrix;
  } else if (matrix === _identity2DMatrix && !target.offsetParent && target !== _docElement && !cache.svg) {
    temp = style.display;
    style.display = "block";
    parent = target.parentNode;
    if (!parent || !target.offsetParent && !target.getBoundingClientRect().width) {
      addedToDOM = 1;
      nextSibling = target.nextElementSibling;
      _docElement.appendChild(target);
    }
    matrix = _getComputedTransformMatrixAsArray(target);
    temp ? style.display = temp : _removeProperty(target, "display");
    if (addedToDOM) {
      nextSibling ? parent.insertBefore(target, nextSibling) : parent ? parent.appendChild(target) : _docElement.removeChild(target);
    }
  }
  return force2D && matrix.length > 6 ? [matrix[0], matrix[1], matrix[4], matrix[5], matrix[12], matrix[13]] : matrix;
};
var _applySVGOrigin = function _applySVGOrigin2(target, origin, originIsAbsolute, smooth, matrixArray, pluginToAddPropTweensTo) {
  var cache = target._gsap, matrix = matrixArray || _getMatrix(target, true), xOriginOld = cache.xOrigin || 0, yOriginOld = cache.yOrigin || 0, xOffsetOld = cache.xOffset || 0, yOffsetOld = cache.yOffset || 0, a = matrix[0], b = matrix[1], c = matrix[2], d = matrix[3], tx = matrix[4], ty = matrix[5], originSplit = origin.split(" "), xOrigin = parseFloat(originSplit[0]) || 0, yOrigin = parseFloat(originSplit[1]) || 0, bounds, determinant, x, y;
  if (!originIsAbsolute) {
    bounds = _getBBox(target);
    xOrigin = bounds.x + (~originSplit[0].indexOf("%") ? xOrigin / 100 * bounds.width : xOrigin);
    yOrigin = bounds.y + (~(originSplit[1] || originSplit[0]).indexOf("%") ? yOrigin / 100 * bounds.height : yOrigin);
  } else if (matrix !== _identity2DMatrix && (determinant = a * d - b * c)) {
    x = xOrigin * (d / determinant) + yOrigin * (-c / determinant) + (c * ty - d * tx) / determinant;
    y = xOrigin * (-b / determinant) + yOrigin * (a / determinant) - (a * ty - b * tx) / determinant;
    xOrigin = x;
    yOrigin = y;
  }
  if (smooth || smooth !== false && cache.smooth) {
    tx = xOrigin - xOriginOld;
    ty = yOrigin - yOriginOld;
    cache.xOffset = xOffsetOld + (tx * a + ty * c) - tx;
    cache.yOffset = yOffsetOld + (tx * b + ty * d) - ty;
  } else {
    cache.xOffset = cache.yOffset = 0;
  }
  cache.xOrigin = xOrigin;
  cache.yOrigin = yOrigin;
  cache.smooth = !!smooth;
  cache.origin = origin;
  cache.originIsAbsolute = !!originIsAbsolute;
  target.style[_transformOriginProp] = "0px 0px";
  if (pluginToAddPropTweensTo) {
    _addNonTweeningPT(pluginToAddPropTweensTo, cache, "xOrigin", xOriginOld, xOrigin);
    _addNonTweeningPT(pluginToAddPropTweensTo, cache, "yOrigin", yOriginOld, yOrigin);
    _addNonTweeningPT(pluginToAddPropTweensTo, cache, "xOffset", xOffsetOld, cache.xOffset);
    _addNonTweeningPT(pluginToAddPropTweensTo, cache, "yOffset", yOffsetOld, cache.yOffset);
  }
  target.setAttribute("data-svg-origin", xOrigin + " " + yOrigin);
};
var _parseTransform = function _parseTransform2(target, uncache) {
  var cache = target._gsap || new GSCache(target);
  if ("x" in cache && !uncache && !cache.uncache) {
    return cache;
  }
  var style = target.style, invertedScaleX = cache.scaleX < 0, px = "px", deg = "deg", cs = getComputedStyle(target), origin = _getComputedProperty(target, _transformOriginProp) || "0", x, y, z, scaleX, scaleY, rotation, rotationX, rotationY, skewX, skewY, perspective, xOrigin, yOrigin, matrix, angle, cos, sin, a, b, c, d, a12, a22, t1, t2, t3, a13, a23, a33, a42, a43, a32;
  x = y = z = rotation = rotationX = rotationY = skewX = skewY = perspective = 0;
  scaleX = scaleY = 1;
  cache.svg = !!(target.getCTM && _isSVG(target));
  if (cs.translate) {
    if (cs.translate !== "none" || cs.scale !== "none" || cs.rotate !== "none") {
      style[_transformProp] = (cs.translate !== "none" ? "translate3d(" + (cs.translate + " 0 0").split(" ").slice(0, 3).join(", ") + ") " : "") + (cs.rotate !== "none" ? "rotate(" + cs.rotate + ") " : "") + (cs.scale !== "none" ? "scale(" + cs.scale.split(" ").join(",") + ") " : "") + (cs[_transformProp] !== "none" ? cs[_transformProp] : "");
    }
    style.scale = style.rotate = style.translate = "none";
  }
  matrix = _getMatrix(target, cache.svg);
  if (cache.svg) {
    if (cache.uncache) {
      t2 = target.getBBox();
      origin = cache.xOrigin - t2.x + "px " + (cache.yOrigin - t2.y) + "px";
      t1 = "";
    } else {
      t1 = !uncache && target.getAttribute("data-svg-origin");
    }
    _applySVGOrigin(target, t1 || origin, !!t1 || cache.originIsAbsolute, cache.smooth !== false, matrix);
  }
  xOrigin = cache.xOrigin || 0;
  yOrigin = cache.yOrigin || 0;
  if (matrix !== _identity2DMatrix) {
    a = matrix[0];
    b = matrix[1];
    c = matrix[2];
    d = matrix[3];
    x = a12 = matrix[4];
    y = a22 = matrix[5];
    if (matrix.length === 6) {
      scaleX = Math.sqrt(a * a + b * b);
      scaleY = Math.sqrt(d * d + c * c);
      rotation = a || b ? _atan2(b, a) * _RAD2DEG : 0;
      skewX = c || d ? _atan2(c, d) * _RAD2DEG + rotation : 0;
      skewX && (scaleY *= Math.abs(Math.cos(skewX * _DEG2RAD)));
      if (cache.svg) {
        x -= xOrigin - (xOrigin * a + yOrigin * c);
        y -= yOrigin - (xOrigin * b + yOrigin * d);
      }
    } else {
      a32 = matrix[6];
      a42 = matrix[7];
      a13 = matrix[8];
      a23 = matrix[9];
      a33 = matrix[10];
      a43 = matrix[11];
      x = matrix[12];
      y = matrix[13];
      z = matrix[14];
      angle = _atan2(a32, a33);
      rotationX = angle * _RAD2DEG;
      if (angle) {
        cos = Math.cos(-angle);
        sin = Math.sin(-angle);
        t1 = a12 * cos + a13 * sin;
        t2 = a22 * cos + a23 * sin;
        t3 = a32 * cos + a33 * sin;
        a13 = a12 * -sin + a13 * cos;
        a23 = a22 * -sin + a23 * cos;
        a33 = a32 * -sin + a33 * cos;
        a43 = a42 * -sin + a43 * cos;
        a12 = t1;
        a22 = t2;
        a32 = t3;
      }
      angle = _atan2(-c, a33);
      rotationY = angle * _RAD2DEG;
      if (angle) {
        cos = Math.cos(-angle);
        sin = Math.sin(-angle);
        t1 = a * cos - a13 * sin;
        t2 = b * cos - a23 * sin;
        t3 = c * cos - a33 * sin;
        a43 = d * sin + a43 * cos;
        a = t1;
        b = t2;
        c = t3;
      }
      angle = _atan2(b, a);
      rotation = angle * _RAD2DEG;
      if (angle) {
        cos = Math.cos(angle);
        sin = Math.sin(angle);
        t1 = a * cos + b * sin;
        t2 = a12 * cos + a22 * sin;
        b = b * cos - a * sin;
        a22 = a22 * cos - a12 * sin;
        a = t1;
        a12 = t2;
      }
      if (rotationX && Math.abs(rotationX) + Math.abs(rotation) > 359.9) {
        rotationX = rotation = 0;
        rotationY = 180 - rotationY;
      }
      scaleX = _round(Math.sqrt(a * a + b * b + c * c));
      scaleY = _round(Math.sqrt(a22 * a22 + a32 * a32));
      angle = _atan2(a12, a22);
      skewX = Math.abs(angle) > 2e-4 ? angle * _RAD2DEG : 0;
      perspective = a43 ? 1 / (a43 < 0 ? -a43 : a43) : 0;
    }
    if (cache.svg) {
      t1 = target.getAttribute("transform");
      cache.forceCSS = target.setAttribute("transform", "") || !_isNullTransform(_getComputedProperty(target, _transformProp));
      t1 && target.setAttribute("transform", t1);
    }
  }
  if (Math.abs(skewX) > 90 && Math.abs(skewX) < 270) {
    if (invertedScaleX) {
      scaleX *= -1;
      skewX += rotation <= 0 ? 180 : -180;
      rotation += rotation <= 0 ? 180 : -180;
    } else {
      scaleY *= -1;
      skewX += skewX <= 0 ? 180 : -180;
    }
  }
  uncache = uncache || cache.uncache;
  cache.x = x - ((cache.xPercent = x && (!uncache && cache.xPercent || (Math.round(target.offsetWidth / 2) === Math.round(-x) ? -50 : 0))) ? target.offsetWidth * cache.xPercent / 100 : 0) + px;
  cache.y = y - ((cache.yPercent = y && (!uncache && cache.yPercent || (Math.round(target.offsetHeight / 2) === Math.round(-y) ? -50 : 0))) ? target.offsetHeight * cache.yPercent / 100 : 0) + px;
  cache.z = z + px;
  cache.scaleX = _round(scaleX);
  cache.scaleY = _round(scaleY);
  cache.rotation = _round(rotation) + deg;
  cache.rotationX = _round(rotationX) + deg;
  cache.rotationY = _round(rotationY) + deg;
  cache.skewX = skewX + deg;
  cache.skewY = skewY + deg;
  cache.transformPerspective = perspective + px;
  if (cache.zOrigin = parseFloat(origin.split(" ")[2]) || !uncache && cache.zOrigin || 0) {
    style[_transformOriginProp] = _firstTwoOnly(origin);
  }
  cache.xOffset = cache.yOffset = 0;
  cache.force3D = _config.force3D;
  cache.renderTransform = cache.svg ? _renderSVGTransforms : _supports3D ? _renderCSSTransforms : _renderNon3DTransforms;
  cache.uncache = 0;
  return cache;
};
var _firstTwoOnly = function _firstTwoOnly2(value) {
  return (value = value.split(" "))[0] + " " + value[1];
};
var _addPxTranslate = function _addPxTranslate2(target, start, value) {
  var unit = getUnit(start);
  return _round(parseFloat(start) + parseFloat(_convertToUnit(target, "x", value + "px", unit))) + unit;
};
var _renderNon3DTransforms = function _renderNon3DTransforms2(ratio, cache) {
  cache.z = "0px";
  cache.rotationY = cache.rotationX = "0deg";
  cache.force3D = 0;
  _renderCSSTransforms(ratio, cache);
};
var _zeroDeg = "0deg";
var _zeroPx = "0px";
var _endParenthesis = ") ";
var _renderCSSTransforms = function _renderCSSTransforms2(ratio, cache) {
  var _ref = cache || this, xPercent = _ref.xPercent, yPercent = _ref.yPercent, x = _ref.x, y = _ref.y, z = _ref.z, rotation = _ref.rotation, rotationY = _ref.rotationY, rotationX = _ref.rotationX, skewX = _ref.skewX, skewY = _ref.skewY, scaleX = _ref.scaleX, scaleY = _ref.scaleY, transformPerspective = _ref.transformPerspective, force3D = _ref.force3D, target = _ref.target, zOrigin = _ref.zOrigin, transforms = "", use3D = force3D === "auto" && ratio && ratio !== 1 || force3D === true;
  if (zOrigin && (rotationX !== _zeroDeg || rotationY !== _zeroDeg)) {
    var angle = parseFloat(rotationY) * _DEG2RAD, a13 = Math.sin(angle), a33 = Math.cos(angle), cos;
    angle = parseFloat(rotationX) * _DEG2RAD;
    cos = Math.cos(angle);
    x = _addPxTranslate(target, x, a13 * cos * -zOrigin);
    y = _addPxTranslate(target, y, -Math.sin(angle) * -zOrigin);
    z = _addPxTranslate(target, z, a33 * cos * -zOrigin + zOrigin);
  }
  if (transformPerspective !== _zeroPx) {
    transforms += "perspective(" + transformPerspective + _endParenthesis;
  }
  if (xPercent || yPercent) {
    transforms += "translate(" + xPercent + "%, " + yPercent + "%) ";
  }
  if (use3D || x !== _zeroPx || y !== _zeroPx || z !== _zeroPx) {
    transforms += z !== _zeroPx || use3D ? "translate3d(" + x + ", " + y + ", " + z + ") " : "translate(" + x + ", " + y + _endParenthesis;
  }
  if (rotation !== _zeroDeg) {
    transforms += "rotate(" + rotation + _endParenthesis;
  }
  if (rotationY !== _zeroDeg) {
    transforms += "rotateY(" + rotationY + _endParenthesis;
  }
  if (rotationX !== _zeroDeg) {
    transforms += "rotateX(" + rotationX + _endParenthesis;
  }
  if (skewX !== _zeroDeg || skewY !== _zeroDeg) {
    transforms += "skew(" + skewX + ", " + skewY + _endParenthesis;
  }
  if (scaleX !== 1 || scaleY !== 1) {
    transforms += "scale(" + scaleX + ", " + scaleY + _endParenthesis;
  }
  target.style[_transformProp] = transforms || "translate(0, 0)";
};
var _renderSVGTransforms = function _renderSVGTransforms2(ratio, cache) {
  var _ref2 = cache || this, xPercent = _ref2.xPercent, yPercent = _ref2.yPercent, x = _ref2.x, y = _ref2.y, rotation = _ref2.rotation, skewX = _ref2.skewX, skewY = _ref2.skewY, scaleX = _ref2.scaleX, scaleY = _ref2.scaleY, target = _ref2.target, xOrigin = _ref2.xOrigin, yOrigin = _ref2.yOrigin, xOffset = _ref2.xOffset, yOffset = _ref2.yOffset, forceCSS = _ref2.forceCSS, tx = parseFloat(x), ty = parseFloat(y), a11, a21, a12, a22, temp;
  rotation = parseFloat(rotation);
  skewX = parseFloat(skewX);
  skewY = parseFloat(skewY);
  if (skewY) {
    skewY = parseFloat(skewY);
    skewX += skewY;
    rotation += skewY;
  }
  if (rotation || skewX) {
    rotation *= _DEG2RAD;
    skewX *= _DEG2RAD;
    a11 = Math.cos(rotation) * scaleX;
    a21 = Math.sin(rotation) * scaleX;
    a12 = Math.sin(rotation - skewX) * -scaleY;
    a22 = Math.cos(rotation - skewX) * scaleY;
    if (skewX) {
      skewY *= _DEG2RAD;
      temp = Math.tan(skewX - skewY);
      temp = Math.sqrt(1 + temp * temp);
      a12 *= temp;
      a22 *= temp;
      if (skewY) {
        temp = Math.tan(skewY);
        temp = Math.sqrt(1 + temp * temp);
        a11 *= temp;
        a21 *= temp;
      }
    }
    a11 = _round(a11);
    a21 = _round(a21);
    a12 = _round(a12);
    a22 = _round(a22);
  } else {
    a11 = scaleX;
    a22 = scaleY;
    a21 = a12 = 0;
  }
  if (tx && !~(x + "").indexOf("px") || ty && !~(y + "").indexOf("px")) {
    tx = _convertToUnit(target, "x", x, "px");
    ty = _convertToUnit(target, "y", y, "px");
  }
  if (xOrigin || yOrigin || xOffset || yOffset) {
    tx = _round(tx + xOrigin - (xOrigin * a11 + yOrigin * a12) + xOffset);
    ty = _round(ty + yOrigin - (xOrigin * a21 + yOrigin * a22) + yOffset);
  }
  if (xPercent || yPercent) {
    temp = target.getBBox();
    tx = _round(tx + xPercent / 100 * temp.width);
    ty = _round(ty + yPercent / 100 * temp.height);
  }
  temp = "matrix(" + a11 + "," + a21 + "," + a12 + "," + a22 + "," + tx + "," + ty + ")";
  target.setAttribute("transform", temp);
  forceCSS && (target.style[_transformProp] = temp);
};
var _addRotationalPropTween = function _addRotationalPropTween2(plugin, target, property, startNum, endValue) {
  var cap = 360, isString = _isString(endValue), endNum = parseFloat(endValue) * (isString && ~endValue.indexOf("rad") ? _RAD2DEG : 1), change = endNum - startNum, finalValue = startNum + change + "deg", direction, pt;
  if (isString) {
    direction = endValue.split("_")[1];
    if (direction === "short") {
      change %= cap;
      if (change !== change % (cap / 2)) {
        change += change < 0 ? cap : -cap;
      }
    }
    if (direction === "cw" && change < 0) {
      change = (change + cap * _bigNum2) % cap - ~~(change / cap) * cap;
    } else if (direction === "ccw" && change > 0) {
      change = (change - cap * _bigNum2) % cap - ~~(change / cap) * cap;
    }
  }
  plugin._pt = pt = new PropTween(plugin._pt, target, property, startNum, change, _renderPropWithEnd);
  pt.e = finalValue;
  pt.u = "deg";
  plugin._props.push(property);
  return pt;
};
var _assign = function _assign2(target, source) {
  for (var p2 in source) {
    target[p2] = source[p2];
  }
  return target;
};
var _addRawTransformPTs = function _addRawTransformPTs2(plugin, transforms, target) {
  var startCache = _assign({}, target._gsap), exclude = "perspective,force3D,transformOrigin,svgOrigin", style = target.style, endCache, p2, startValue, endValue, startNum, endNum, startUnit, endUnit;
  if (startCache.svg) {
    startValue = target.getAttribute("transform");
    target.setAttribute("transform", "");
    style[_transformProp] = transforms;
    endCache = _parseTransform(target, 1);
    _removeProperty(target, _transformProp);
    target.setAttribute("transform", startValue);
  } else {
    startValue = getComputedStyle(target)[_transformProp];
    style[_transformProp] = transforms;
    endCache = _parseTransform(target, 1);
    style[_transformProp] = startValue;
  }
  for (p2 in _transformProps) {
    startValue = startCache[p2];
    endValue = endCache[p2];
    if (startValue !== endValue && exclude.indexOf(p2) < 0) {
      startUnit = getUnit(startValue);
      endUnit = getUnit(endValue);
      startNum = startUnit !== endUnit ? _convertToUnit(target, p2, startValue, endUnit) : parseFloat(startValue);
      endNum = parseFloat(endValue);
      plugin._pt = new PropTween(plugin._pt, endCache, p2, startNum, endNum - startNum, _renderCSSProp);
      plugin._pt.u = endUnit || 0;
      plugin._props.push(p2);
    }
  }
  _assign(endCache, startCache);
};
_forEachName("padding,margin,Width,Radius", function(name, index) {
  var t = "Top", r = "Right", b = "Bottom", l = "Left", props = (index < 3 ? [t, r, b, l] : [t + l, t + r, b + r, b + l]).map(function(side) {
    return index < 2 ? name + side : "border" + side + name;
  });
  _specialProps[index > 1 ? "border" + name : name] = function(plugin, target, property, endValue, tween) {
    var a, vars;
    if (arguments.length < 4) {
      a = props.map(function(prop) {
        return _get(plugin, prop, property);
      });
      vars = a.join(" ");
      return vars.split(a[0]).length === 5 ? a[0] : vars;
    }
    a = (endValue + "").split(" ");
    vars = {};
    props.forEach(function(prop, i2) {
      return vars[prop] = a[i2] = a[i2] || a[(i2 - 1) / 2 | 0];
    });
    plugin.init(target, vars, tween);
  };
});
var CSSPlugin = {
  name: "css",
  register: _initCore,
  targetTest: function targetTest(target) {
    return target.style && target.nodeType;
  },
  init: function init3(target, vars, tween, index, targets) {
    var props = this._props, style = target.style, startAt = tween.vars.startAt, startValue, endValue, endNum, startNum, type, specialProp, p2, startUnit, endUnit, relative, isTransformRelated, transformPropTween, cache, smooth, hasPriority, inlineProps, finalTransformValue;
    _pluginInitted || _initCore();
    this.styles = this.styles || _getStyleSaver(target);
    inlineProps = this.styles.props;
    this.tween = tween;
    for (p2 in vars) {
      if (p2 === "autoRound") {
        continue;
      }
      endValue = vars[p2];
      if (_plugins[p2] && _checkPlugin(p2, vars, tween, index, target, targets)) {
        continue;
      }
      type = typeof endValue;
      specialProp = _specialProps[p2];
      if (type === "function") {
        endValue = endValue.call(tween, index, target, targets);
        type = typeof endValue;
      }
      if (type === "string" && ~endValue.indexOf("random(")) {
        endValue = _replaceRandom(endValue);
      }
      if (specialProp) {
        specialProp(this, target, p2, endValue, tween) && (hasPriority = 1);
      } else if (p2.substr(0, 2) === "--") {
        startValue = (getComputedStyle(target).getPropertyValue(p2) + "").trim();
        endValue += "";
        _colorExp.lastIndex = 0;
        if (!_colorExp.test(startValue)) {
          startUnit = getUnit(startValue);
          endUnit = getUnit(endValue);
          endUnit ? startUnit !== endUnit && (startValue = _convertToUnit(target, p2, startValue, endUnit) + endUnit) : startUnit && (endValue += startUnit);
        }
        this.add(style, "setProperty", startValue, endValue, index, targets, 0, 0, p2);
        props.push(p2);
        inlineProps.push(p2, 0, style[p2]);
      } else if (type !== "undefined") {
        if (startAt && p2 in startAt) {
          startValue = typeof startAt[p2] === "function" ? startAt[p2].call(tween, index, target, targets) : startAt[p2];
          _isString(startValue) && ~startValue.indexOf("random(") && (startValue = _replaceRandom(startValue));
          getUnit(startValue + "") || startValue === "auto" || (startValue += _config.units[p2] || getUnit(_get(target, p2)) || "");
          (startValue + "").charAt(1) === "=" && (startValue = _get(target, p2));
        } else {
          startValue = _get(target, p2);
        }
        startNum = parseFloat(startValue);
        relative = type === "string" && endValue.charAt(1) === "=" && endValue.substr(0, 2);
        relative && (endValue = endValue.substr(2));
        endNum = parseFloat(endValue);
        if (p2 in _propertyAliases) {
          if (p2 === "autoAlpha") {
            if (startNum === 1 && _get(target, "visibility") === "hidden" && endNum) {
              startNum = 0;
            }
            inlineProps.push("visibility", 0, style.visibility);
            _addNonTweeningPT(this, style, "visibility", startNum ? "inherit" : "hidden", endNum ? "inherit" : "hidden", !endNum);
          }
          if (p2 !== "scale" && p2 !== "transform") {
            p2 = _propertyAliases[p2];
            ~p2.indexOf(",") && (p2 = p2.split(",")[0]);
          }
        }
        isTransformRelated = p2 in _transformProps;
        if (isTransformRelated) {
          this.styles.save(p2);
          finalTransformValue = endValue;
          if (type === "string" && endValue.substring(0, 6) === "var(--") {
            endValue = _getComputedProperty(target, endValue.substring(4, endValue.indexOf(")")));
            if (endValue.substring(0, 5) === "calc(") {
              var origPerspective = target.style.perspective;
              target.style.perspective = endValue;
              endValue = _getComputedProperty(target, "perspective");
              origPerspective ? target.style.perspective = origPerspective : _removeProperty(target, "perspective");
            }
            endNum = parseFloat(endValue);
          }
          if (!transformPropTween) {
            cache = target._gsap;
            cache.renderTransform && !vars.parseTransform || _parseTransform(target, vars.parseTransform);
            smooth = vars.smoothOrigin !== false && cache.smooth;
            transformPropTween = this._pt = new PropTween(this._pt, style, _transformProp, 0, 1, cache.renderTransform, cache, 0, -1);
            transformPropTween.dep = 1;
          }
          if (p2 === "scale") {
            this._pt = new PropTween(this._pt, cache, "scaleY", cache.scaleY, (relative ? _parseRelative(cache.scaleY, relative + endNum) : endNum) - cache.scaleY || 0, _renderCSSProp);
            this._pt.u = 0;
            props.push("scaleY", p2);
            p2 += "X";
          } else if (p2 === "transformOrigin") {
            inlineProps.push(_transformOriginProp, 0, style[_transformOriginProp]);
            endValue = _convertKeywordsToPercentages(endValue);
            if (cache.svg) {
              _applySVGOrigin(target, endValue, 0, smooth, 0, this);
            } else {
              endUnit = parseFloat(endValue.split(" ")[2]) || 0;
              endUnit !== cache.zOrigin && _addNonTweeningPT(this, cache, "zOrigin", cache.zOrigin, endUnit);
              _addNonTweeningPT(this, style, p2, _firstTwoOnly(startValue), _firstTwoOnly(endValue));
            }
            continue;
          } else if (p2 === "svgOrigin") {
            _applySVGOrigin(target, endValue, 1, smooth, 0, this);
            continue;
          } else if (p2 in _rotationalProperties) {
            _addRotationalPropTween(this, cache, p2, startNum, relative ? _parseRelative(startNum, relative + endValue) : endValue);
            continue;
          } else if (p2 === "smoothOrigin") {
            _addNonTweeningPT(this, cache, "smooth", cache.smooth, endValue);
            continue;
          } else if (p2 === "force3D") {
            cache[p2] = endValue;
            continue;
          } else if (p2 === "transform") {
            _addRawTransformPTs(this, endValue, target);
            continue;
          }
        } else if (!(p2 in style)) {
          p2 = _checkPropPrefix(p2) || p2;
        }
        if (isTransformRelated || (endNum || endNum === 0) && (startNum || startNum === 0) && !_complexExp.test(endValue) && p2 in style) {
          startUnit = (startValue + "").substr((startNum + "").length);
          endNum || (endNum = 0);
          endUnit = getUnit(endValue) || (p2 in _config.units ? _config.units[p2] : startUnit);
          startUnit !== endUnit && (startNum = _convertToUnit(target, p2, startValue, endUnit));
          this._pt = new PropTween(this._pt, isTransformRelated ? cache : style, p2, startNum, (relative ? _parseRelative(startNum, relative + endNum) : endNum) - startNum, !isTransformRelated && (endUnit === "px" || p2 === "zIndex") && vars.autoRound !== false ? _renderRoundedCSSProp : _renderCSSProp);
          this._pt.u = endUnit || 0;
          if (isTransformRelated && finalTransformValue !== endValue) {
            this._pt.b = startValue;
            this._pt.e = finalTransformValue;
            this._pt.r = _renderCSSPropWithBeginningAndEnd;
          } else if (startUnit !== endUnit && endUnit !== "%") {
            this._pt.b = startValue;
            this._pt.r = _renderCSSPropWithBeginning;
          }
        } else if (!(p2 in style)) {
          if (p2 in target) {
            this.add(target, p2, startValue || target[p2], relative ? relative + endValue : endValue, index, targets);
          } else if (p2 !== "parseTransform") {
            _missingPlugin(p2, endValue);
            continue;
          }
        } else {
          _tweenComplexCSSString.call(this, target, p2, startValue, relative ? relative + endValue : endValue);
        }
        isTransformRelated || (p2 in style ? inlineProps.push(p2, 0, style[p2]) : typeof target[p2] === "function" ? inlineProps.push(p2, 2, target[p2]()) : inlineProps.push(p2, 1, startValue || target[p2]));
        props.push(p2);
      }
    }
    hasPriority && _sortPropTweensByPriority(this);
  },
  render: function render2(ratio, data) {
    if (data.tween._time || !_reverting2()) {
      var pt = data._pt;
      while (pt) {
        pt.r(ratio, pt.d);
        pt = pt._next;
      }
    } else {
      data.styles.revert();
    }
  },
  get: _get,
  aliases: _propertyAliases,
  getSetter: function getSetter(target, property, plugin) {
    var p2 = _propertyAliases[property];
    p2 && p2.indexOf(",") < 0 && (property = p2);
    return property in _transformProps && property !== _transformOriginProp && (target._gsap.x || _get(target, "x")) ? plugin && _recentSetterPlugin === plugin ? property === "scale" ? _setterScale : _setterTransform : (_recentSetterPlugin = plugin || {}) && (property === "scale" ? _setterScaleWithRender : _setterTransformWithRender) : target.style && !_isUndefined(target.style[property]) ? _setterCSSStyle : ~property.indexOf("-") ? _setterCSSProp : _getSetter(target, property);
  },
  core: {
    _removeProperty,
    _getMatrix
  }
};
gsap.utils.checkPrefix = _checkPropPrefix;
gsap.core.getStyleSaver = _getStyleSaver;
(function(positionAndScale, rotation, others, aliases) {
  var all = _forEachName(positionAndScale + "," + rotation + "," + others, function(name) {
    _transformProps[name] = 1;
  });
  _forEachName(rotation, function(name) {
    _config.units[name] = "deg";
    _rotationalProperties[name] = 1;
  });
  _propertyAliases[all[13]] = positionAndScale + "," + rotation;
  _forEachName(aliases, function(name) {
    var split = name.split(":");
    _propertyAliases[split[1]] = all[split[0]];
  });
})("x,y,z,scale,scaleX,scaleY,xPercent,yPercent", "rotation,rotationX,rotationY,skewX,skewY", "transform,transformOrigin,svgOrigin,force3D,smoothOrigin,transformPerspective", "0:translateX,1:translateY,2:translateZ,8:rotate,8:rotationZ,8:rotateZ,9:rotateX,10:rotateY");
_forEachName("x,y,z,top,right,bottom,left,width,height,fontSize,padding,margin,perspective", function(name) {
  _config.units[name] = "px";
});
gsap.registerPlugin(CSSPlugin);

// examples/figma-launch/node_modules/gsap/index.js
var gsapWithCSS = gsap.registerPlugin(CSSPlugin) || gsap;
var TweenMaxWithCSS = gsapWithCSS.core.Tween;

// examples/figma-launch/node_modules/gsap/utils/paths.js
var _svgPathExp = /[achlmqstvz]|(-?\d*\.?\d*(?:e[\-+]?\d+)?)[0-9]/ig;
var _numbersExp = /(?:(-)?\d*\.?\d*(?:e[\-+]?\d+)?)[0-9]/ig;
var _scientific = /[\+\-]?\d*\.?\d+e[\+\-]?\d+/ig;
var _selectorExp = /(^[#\.][a-z]|[a-y][a-z])/i;
var _DEG2RAD2 = Math.PI / 180;
var _RAD2DEG2 = 180 / Math.PI;
var _sin2 = Math.sin;
var _cos2 = Math.cos;
var _abs = Math.abs;
var _sqrt2 = Math.sqrt;
var _atan22 = Math.atan2;
var _largeNum = 1e8;
var _isString3 = function _isString4(value) {
  return typeof value === "string";
};
var _isNumber3 = function _isNumber4(value) {
  return typeof value === "number";
};
var _isUndefined3 = function _isUndefined4(value) {
  return typeof value === "undefined";
};
var _temp = {};
var _temp2 = {};
var _roundingNum = 1e5;
var _wrapProgress = function _wrapProgress2(progress) {
  return Math.round((progress + _largeNum) % 1 * _roundingNum) / _roundingNum || (progress < 0 ? 0 : 1);
};
var _round3 = function _round4(value) {
  return Math.round(value * _roundingNum) / _roundingNum || 0;
};
var _roundPrecise3 = function _roundPrecise4(value) {
  return Math.round(value * 1e10) / 1e10 || 0;
};
var _segmentIsClosed = function _segmentIsClosed2(segment) {
  return segment.closed = Math.abs(segment[0] - segment[segment.length - 2]) < 1e-3 && Math.abs(segment[1] - segment[segment.length - 1]) < 1e-3;
};
var _splitSegment = function _splitSegment2(rawPath, segIndex, i2, t) {
  var segment = rawPath[segIndex], shift = t === 1 ? 6 : subdivideSegment(segment, i2, t);
  if ((shift || !t) && shift + i2 + 2 < segment.length) {
    rawPath.splice(segIndex, 0, segment.slice(0, i2 + shift + 2));
    segment.splice(0, i2 + shift);
    return 1;
  }
};
var _getSampleIndex = function _getSampleIndex2(samples, length, progress) {
  var l = samples.length, i2 = ~~(progress * l);
  if (samples[i2] > length) {
    while (--i2 && samples[i2] > length) {
    }
    i2 < 0 && (i2 = 0);
  } else {
    while (samples[++i2] < length && i2 < l) {
    }
  }
  return i2 < l ? i2 : l - 1;
};
var _reverseRawPath = function _reverseRawPath2(rawPath, skipOuter) {
  var i2 = rawPath.length;
  skipOuter || rawPath.reverse();
  while (i2--) {
    rawPath[i2].reversed || reverseSegment(rawPath[i2]);
  }
};
var _copyMetaData = function _copyMetaData2(source, copy) {
  copy.totalLength = source.totalLength;
  if (source.samples) {
    copy.samples = source.samples.slice(0);
    copy.lookup = source.lookup.slice(0);
    copy.minLength = source.minLength;
    copy.resolution = source.resolution;
  } else if (source.totalPoints) {
    copy.totalPoints = source.totalPoints;
  }
  return copy;
};
var _appendOrMerge = function _appendOrMerge2(rawPath, segment) {
  var index = rawPath.length, prevSeg = rawPath[index - 1] || [], l = prevSeg.length;
  if (index && segment[0] === prevSeg[l - 2] && segment[1] === prevSeg[l - 1]) {
    segment = prevSeg.concat(segment.slice(2));
    index--;
  }
  rawPath[index] = segment;
};
var _bestDistance;
function getRawPath(value) {
  value = _isString3(value) && _selectorExp.test(value) ? document.querySelector(value) || value : value;
  var e = value.getAttribute ? value : 0, rawPath;
  if (e && (value = value.getAttribute("d"))) {
    if (!e._gsPath) {
      e._gsPath = {};
    }
    rawPath = e._gsPath[value];
    return rawPath && !rawPath._dirty ? rawPath : e._gsPath[value] = stringToRawPath(value);
  }
  return !value ? console.warn("Expecting a <path> element or an SVG path data string") : _isString3(value) ? stringToRawPath(value) : _isNumber3(value[0]) ? [value] : value;
}
function copyRawPath(rawPath) {
  var a = [], i2 = 0;
  for (; i2 < rawPath.length; i2++) {
    a[i2] = _copyMetaData(rawPath[i2], rawPath[i2].slice(0));
  }
  return _copyMetaData(rawPath, a);
}
function reverseSegment(segment) {
  var i2 = 0, y;
  segment.reverse();
  for (; i2 < segment.length; i2 += 2) {
    y = segment[i2];
    segment[i2] = segment[i2 + 1];
    segment[i2 + 1] = y;
  }
  segment.reversed = !segment.reversed;
}
var _createPath = function _createPath2(e, ignore) {
  var path = document.createElementNS("http://www.w3.org/2000/svg", "path"), attr = [].slice.call(e.attributes), i2 = attr.length, name;
  ignore = "," + ignore + ",";
  while (--i2 > -1) {
    name = attr[i2].nodeName.toLowerCase();
    if (ignore.indexOf("," + name + ",") < 0) {
      path.setAttributeNS(null, name, attr[i2].nodeValue);
    }
  }
  return path;
};
var _typeAttrs = {
  rect: "rx,ry,x,y,width,height",
  circle: "r,cx,cy",
  ellipse: "rx,ry,cx,cy",
  line: "x1,x2,y1,y2"
};
var _attrToObj = function _attrToObj2(e, attrs) {
  var props = attrs ? attrs.split(",") : [], obj = {}, i2 = props.length;
  while (--i2 > -1) {
    obj[props[i2]] = +e.getAttribute(props[i2]) || 0;
  }
  return obj;
};
function convertToPath(element, swap) {
  var type = element.tagName.toLowerCase(), circ = 0.552284749831, data, x, y, r, ry, path, rcirc, rycirc, points, w, h, x2, x3, x4, x5, x6, y2, y3, y4, y5, y6, attr;
  if (type === "path" || !element.getBBox) {
    return element;
  }
  path = _createPath(element, "x,y,width,height,cx,cy,rx,ry,r,x1,x2,y1,y2,points");
  attr = _attrToObj(element, _typeAttrs[type]);
  if (type === "rect") {
    r = attr.rx;
    ry = attr.ry || r;
    x = attr.x;
    y = attr.y;
    w = attr.width - r * 2;
    h = attr.height - ry * 2;
    if (r || ry) {
      x2 = x + r * (1 - circ);
      x3 = x + r;
      x4 = x3 + w;
      x5 = x4 + r * circ;
      x6 = x4 + r;
      y2 = y + ry * (1 - circ);
      y3 = y + ry;
      y4 = y3 + h;
      y5 = y4 + ry * circ;
      y6 = y4 + ry;
      data = "M" + x6 + "," + y3 + " V" + y4 + " C" + [x6, y5, x5, y6, x4, y6, x4 - (x4 - x3) / 3, y6, x3 + (x4 - x3) / 3, y6, x3, y6, x2, y6, x, y5, x, y4, x, y4 - (y4 - y3) / 3, x, y3 + (y4 - y3) / 3, x, y3, x, y2, x2, y, x3, y, x3 + (x4 - x3) / 3, y, x4 - (x4 - x3) / 3, y, x4, y, x5, y, x6, y2, x6, y3].join(",") + "z";
    } else {
      data = "M" + (x + w) + "," + y + " v" + h + " h" + -w + " v" + -h + " h" + w + "z";
    }
  } else if (type === "circle" || type === "ellipse") {
    if (type === "circle") {
      r = ry = attr.r;
      rycirc = r * circ;
    } else {
      r = attr.rx;
      ry = attr.ry;
      rycirc = ry * circ;
    }
    x = attr.cx;
    y = attr.cy;
    rcirc = r * circ;
    data = "M" + (x + r) + "," + y + " C" + [x + r, y + rycirc, x + rcirc, y + ry, x, y + ry, x - rcirc, y + ry, x - r, y + rycirc, x - r, y, x - r, y - rycirc, x - rcirc, y - ry, x, y - ry, x + rcirc, y - ry, x + r, y - rycirc, x + r, y].join(",") + "z";
  } else if (type === "line") {
    data = "M" + attr.x1 + "," + attr.y1 + " L" + attr.x2 + "," + attr.y2;
  } else if (type === "polyline" || type === "polygon") {
    points = (element.getAttribute("points") + "").match(_numbersExp) || [];
    x = points.shift();
    y = points.shift();
    data = "M" + x + "," + y + " L" + points.join(",");
    if (type === "polygon") {
      data += "," + x + "," + y + "z";
    }
  }
  path.setAttribute("d", rawPathToString(path._gsRawPath = stringToRawPath(data)));
  if (swap && element.parentNode) {
    element.parentNode.insertBefore(path, element);
    element.parentNode.removeChild(element);
  }
  return path;
}
function getRotationAtBezierT(segment, i2, t) {
  var a = segment[i2], b = segment[i2 + 2], c = segment[i2 + 4], x;
  a += (b - a) * t;
  b += (c - b) * t;
  a += (b - a) * t;
  x = b + (c + (segment[i2 + 6] - c) * t - b) * t - a;
  a = segment[i2 + 1];
  b = segment[i2 + 3];
  c = segment[i2 + 5];
  a += (b - a) * t;
  b += (c - b) * t;
  a += (b - a) * t;
  return _round3(_atan22(b + (c + (segment[i2 + 7] - c) * t - b) * t - a, x) * _RAD2DEG2);
}
function sliceRawPath(rawPath, start, end) {
  end = _isUndefined3(end) ? 1 : _roundPrecise3(end) || 0;
  start = _roundPrecise3(start) || 0;
  var loops = Math.max(0, ~~(_abs(end - start) - 1e-8)), path = copyRawPath(rawPath);
  if (start > end) {
    start = 1 - start;
    end = 1 - end;
    _reverseRawPath(path);
    path.totalLength = 0;
  }
  if (start < 0 || end < 0) {
    var offset = Math.abs(~~Math.min(start, end)) + 1;
    start += offset;
    end += offset;
  }
  path.totalLength || cacheRawPathMeasurements(path);
  var wrap3 = end > 1, s = getProgressData(path, start, _temp, true), e = getProgressData(path, end, _temp2), eSeg = e.segment, sSeg = s.segment, eSegIndex = e.segIndex, sSegIndex = s.segIndex, ei = e.i, si = s.i, sameSegment = sSegIndex === eSegIndex, sameBezier = ei === si && sameSegment, wrapsBehind, sShift, eShift, i2, copy, totalSegments, l, j;
  if (wrap3 || loops) {
    wrapsBehind = eSegIndex < sSegIndex || sameSegment && ei < si || sameBezier && e.t < s.t;
    if (_splitSegment(path, sSegIndex, si, s.t)) {
      sSegIndex++;
      if (!wrapsBehind) {
        eSegIndex++;
        if (sameBezier) {
          e.t = (e.t - s.t) / (1 - s.t);
          ei = 0;
        } else if (sameSegment) {
          ei -= si;
        }
      }
    }
    if (Math.abs(1 - (end - start)) < 1e-5) {
      eSegIndex = sSegIndex - 1;
    } else if (!e.t && eSegIndex) {
      eSegIndex--;
    } else if (_splitSegment(path, eSegIndex, ei, e.t) && wrapsBehind) {
      sSegIndex++;
    }
    if (s.t === 1) {
      sSegIndex = (sSegIndex + 1) % path.length;
    }
    copy = [];
    totalSegments = path.length;
    l = 1 + totalSegments * loops;
    j = sSegIndex;
    l += (totalSegments - sSegIndex + eSegIndex) % totalSegments;
    for (i2 = 0; i2 < l; i2++) {
      _appendOrMerge(copy, path[j++ % totalSegments]);
    }
    path = copy;
  } else {
    eShift = e.t === 1 ? 6 : subdivideSegment(eSeg, ei, e.t);
    if (start !== end) {
      sShift = subdivideSegment(sSeg, si, sameBezier ? s.t / e.t : s.t);
      sameSegment && (eShift += sShift);
      eSeg.splice(ei + eShift + 2);
      (sShift || si) && sSeg.splice(0, si + sShift);
      i2 = path.length;
      while (i2--) {
        (i2 < sSegIndex || i2 > eSegIndex) && path.splice(i2, 1);
      }
    } else {
      eSeg.angle = getRotationAtBezierT(eSeg, ei + eShift, 0);
      ei += eShift;
      s = eSeg[ei];
      e = eSeg[ei + 1];
      eSeg.length = eSeg.totalLength = 0;
      eSeg.totalPoints = path.totalPoints = 8;
      eSeg.push(s, e, s, e, s, e, s, e);
    }
  }
  path.totalLength = 0;
  return path;
}
function measureSegment(segment, startIndex, bezierQty) {
  startIndex = startIndex || 0;
  if (!segment.samples) {
    segment.samples = [];
    segment.lookup = [];
  }
  var resolution = ~~segment.resolution || 12, inc = 1 / resolution, endIndex = bezierQty ? startIndex + bezierQty * 6 + 1 : segment.length, x1 = segment[startIndex], y1 = segment[startIndex + 1], samplesIndex = startIndex ? startIndex / 6 * resolution : 0, samples = segment.samples, lookup = segment.lookup, min = (startIndex ? segment.minLength : _largeNum) || _largeNum, prevLength = samples[samplesIndex + bezierQty * resolution - 1], length = startIndex ? samples[samplesIndex - 1] : 0, i2, j, x4, x3, x2, xd, xd1, y4, y3, y2, yd, yd1, inv, t, lengthIndex, l, segLength;
  samples.length = lookup.length = 0;
  for (j = startIndex + 2; j < endIndex; j += 6) {
    x4 = segment[j + 4] - x1;
    x3 = segment[j + 2] - x1;
    x2 = segment[j] - x1;
    y4 = segment[j + 5] - y1;
    y3 = segment[j + 3] - y1;
    y2 = segment[j + 1] - y1;
    xd = xd1 = yd = yd1 = 0;
    if (_abs(x4) < 0.01 && _abs(y4) < 0.01 && _abs(x2) + _abs(y2) < 0.01) {
      if (segment.length > 8) {
        segment.splice(j, 6);
        j -= 6;
        endIndex -= 6;
      }
    } else {
      for (i2 = 1; i2 <= resolution; i2++) {
        t = inc * i2;
        inv = 1 - t;
        xd = xd1 - (xd1 = (t * t * x4 + 3 * inv * (t * x3 + inv * x2)) * t);
        yd = yd1 - (yd1 = (t * t * y4 + 3 * inv * (t * y3 + inv * y2)) * t);
        l = _sqrt2(yd * yd + xd * xd);
        if (l < min) {
          min = l;
        }
        length += l;
        samples[samplesIndex++] = length;
      }
    }
    x1 += x4;
    y1 += y4;
  }
  if (prevLength) {
    prevLength -= length;
    for (; samplesIndex < samples.length; samplesIndex++) {
      samples[samplesIndex] += prevLength;
    }
  }
  if (samples.length && min) {
    segment.totalLength = segLength = samples[samples.length - 1] || 0;
    segment.minLength = min;
    if (segLength / min < 9999) {
      l = lengthIndex = 0;
      for (i2 = 0; i2 < segLength; i2 += min) {
        lookup[l++] = samples[lengthIndex] < i2 ? ++lengthIndex : lengthIndex;
      }
    }
  } else {
    segment.totalLength = samples[0] = 0;
  }
  return startIndex ? length - samples[startIndex / 2 - 1] : length;
}
function cacheRawPathMeasurements(rawPath, resolution) {
  var pathLength, points, i2;
  for (i2 = pathLength = points = 0; i2 < rawPath.length; i2++) {
    rawPath[i2].resolution = ~~resolution || 12;
    pathLength += measureSegment(rawPath[i2]);
    points += rawPath[i2].length;
  }
  rawPath.totalPoints = points;
  rawPath.totalLength = pathLength;
  return rawPath;
}
function subdivideSegment(segment, i2, t) {
  if (t <= 0 || t >= 1) {
    return 0;
  }
  var ax = segment[i2], ay = segment[i2 + 1], cp1x = segment[i2 + 2], cp1y = segment[i2 + 3], cp2x = segment[i2 + 4], cp2y = segment[i2 + 5], bx = segment[i2 + 6], by = segment[i2 + 7], x1a = ax + (cp1x - ax) * t, x2 = cp1x + (cp2x - cp1x) * t, y1a = ay + (cp1y - ay) * t, y2 = cp1y + (cp2y - cp1y) * t, x1 = x1a + (x2 - x1a) * t, y1 = y1a + (y2 - y1a) * t, x2a = cp2x + (bx - cp2x) * t, y2a = cp2y + (by - cp2y) * t;
  x2 += (x2a - x2) * t;
  y2 += (y2a - y2) * t;
  segment.splice(
    i2 + 2,
    4,
    _round3(x1a),
    //first control point
    _round3(y1a),
    _round3(x1),
    //second control point
    _round3(y1),
    _round3(x1 + (x2 - x1) * t),
    //new fabricated anchor on line
    _round3(y1 + (y2 - y1) * t),
    _round3(x2),
    //third control point
    _round3(y2),
    _round3(x2a),
    //fourth control point
    _round3(y2a)
  );
  segment.samples && segment.samples.splice(i2 / 6 * segment.resolution | 0, 0, 0, 0, 0, 0, 0, 0);
  return 6;
}
function getProgressData(rawPath, progress, decoratee, pushToNextIfAtEnd) {
  decoratee = decoratee || {};
  rawPath.totalLength || cacheRawPathMeasurements(rawPath);
  if (progress < 0 || progress > 1) {
    progress = _wrapProgress(progress);
  }
  var segIndex = 0, segment = rawPath[0], samples, resolution, length, min, max, i2, t;
  if (!progress) {
    t = i2 = segIndex = 0;
    segment = rawPath[0];
  } else if (progress === 1) {
    t = 1;
    segIndex = rawPath.length - 1;
    segment = rawPath[segIndex];
    i2 = segment.length - 8;
  } else {
    if (rawPath.length > 1) {
      length = rawPath.totalLength * progress;
      max = i2 = 0;
      while ((max += rawPath[i2++].totalLength) < length) {
        segIndex = i2;
      }
      segment = rawPath[segIndex];
      min = max - segment.totalLength;
      progress = (length - min) / (max - min) || 0;
    }
    samples = segment.samples;
    resolution = segment.resolution;
    length = segment.totalLength * progress;
    i2 = segment.lookup.length ? segment.lookup[~~(length / segment.minLength)] || 0 : _getSampleIndex(samples, length, progress);
    min = i2 ? samples[i2 - 1] : 0;
    max = samples[i2];
    if (max < length) {
      min = max;
      max = samples[++i2];
    }
    t = 1 / resolution * ((length - min) / (max - min) + i2 % resolution);
    i2 = ~~(i2 / resolution) * 6;
    if (pushToNextIfAtEnd && t === 1) {
      if (i2 + 6 < segment.length) {
        i2 += 6;
        t = 0;
      } else if (segIndex + 1 < rawPath.length) {
        i2 = t = 0;
        segment = rawPath[++segIndex];
      }
    }
  }
  decoratee.t = t;
  decoratee.i = i2;
  decoratee.path = rawPath;
  decoratee.segment = segment;
  decoratee.segIndex = segIndex;
  return decoratee;
}
function getPositionOnPath(rawPath, progress, includeAngle, point) {
  var segment = rawPath[0], result = point || {}, samples, resolution, length, min, max, i2, t, a, inv;
  if (progress < 0 || progress > 1) {
    progress = _wrapProgress(progress);
  }
  segment.lookup || cacheRawPathMeasurements(rawPath);
  if (rawPath.length > 1) {
    length = rawPath.totalLength * progress;
    max = i2 = 0;
    while ((max += rawPath[i2++].totalLength) < length) {
      segment = rawPath[i2];
    }
    min = max - segment.totalLength;
    progress = (length - min) / (max - min) || 0;
  }
  samples = segment.samples;
  resolution = segment.resolution;
  length = segment.totalLength * progress;
  i2 = segment.lookup.length ? segment.lookup[progress < 1 ? ~~(length / segment.minLength) : segment.lookup.length - 1] || 0 : _getSampleIndex(samples, length, progress);
  min = i2 ? samples[i2 - 1] : 0;
  max = samples[i2];
  if (max < length) {
    min = max;
    max = samples[++i2];
  }
  t = 1 / resolution * ((length - min) / (max - min) + i2 % resolution) || 0;
  inv = 1 - t;
  i2 = ~~(i2 / resolution) * 6;
  a = segment[i2];
  result.x = _round3((t * t * (segment[i2 + 6] - a) + 3 * inv * (t * (segment[i2 + 4] - a) + inv * (segment[i2 + 2] - a))) * t + a);
  result.y = _round3((t * t * (segment[i2 + 7] - (a = segment[i2 + 1])) + 3 * inv * (t * (segment[i2 + 5] - a) + inv * (segment[i2 + 3] - a))) * t + a);
  if (includeAngle) {
    result.angle = segment.totalLength ? getRotationAtBezierT(segment, i2, t >= 1 ? 1 - 1e-9 : t ? t : 1e-9) : segment.angle || 0;
  }
  return result;
}
function transformRawPath(rawPath, a, b, c, d, tx, ty) {
  var j = rawPath.length, segment, l, i2, x, y;
  while (--j > -1) {
    segment = rawPath[j];
    l = segment.length;
    for (i2 = 0; i2 < l; i2 += 2) {
      x = segment[i2];
      y = segment[i2 + 1];
      segment[i2] = x * a + y * c + tx;
      segment[i2 + 1] = x * b + y * d + ty;
    }
  }
  rawPath._dirty = 1;
  return rawPath;
}
function arcToSegment(lastX, lastY, rx, ry, angle, largeArcFlag, sweepFlag, x, y) {
  if (lastX === x && lastY === y) {
    return;
  }
  rx = _abs(rx);
  ry = _abs(ry);
  var angleRad = angle % 360 * _DEG2RAD2, cosAngle = _cos2(angleRad), sinAngle = _sin2(angleRad), PI = Math.PI, TWOPI = PI * 2, dx2 = (lastX - x) / 2, dy2 = (lastY - y) / 2, x1 = cosAngle * dx2 + sinAngle * dy2, y1 = -sinAngle * dx2 + cosAngle * dy2, x1_sq = x1 * x1, y1_sq = y1 * y1, radiiCheck = x1_sq / (rx * rx) + y1_sq / (ry * ry);
  if (radiiCheck > 1) {
    rx = _sqrt2(radiiCheck) * rx;
    ry = _sqrt2(radiiCheck) * ry;
  }
  var rx_sq = rx * rx, ry_sq = ry * ry, sq = (rx_sq * ry_sq - rx_sq * y1_sq - ry_sq * x1_sq) / (rx_sq * y1_sq + ry_sq * x1_sq);
  if (sq < 0) {
    sq = 0;
  }
  var coef = (largeArcFlag === sweepFlag ? -1 : 1) * _sqrt2(sq), cx1 = coef * (rx * y1 / ry), cy1 = coef * -(ry * x1 / rx), sx2 = (lastX + x) / 2, sy2 = (lastY + y) / 2, cx = sx2 + (cosAngle * cx1 - sinAngle * cy1), cy = sy2 + (sinAngle * cx1 + cosAngle * cy1), ux = (x1 - cx1) / rx, uy = (y1 - cy1) / ry, vx = (-x1 - cx1) / rx, vy = (-y1 - cy1) / ry, temp = ux * ux + uy * uy, angleStart = (uy < 0 ? -1 : 1) * Math.acos(ux / _sqrt2(temp)), angleExtent = (ux * vy - uy * vx < 0 ? -1 : 1) * Math.acos((ux * vx + uy * vy) / _sqrt2(temp * (vx * vx + vy * vy)));
  isNaN(angleExtent) && (angleExtent = PI);
  if (!sweepFlag && angleExtent > 0) {
    angleExtent -= TWOPI;
  } else if (sweepFlag && angleExtent < 0) {
    angleExtent += TWOPI;
  }
  angleStart %= TWOPI;
  angleExtent %= TWOPI;
  var segments = Math.ceil(_abs(angleExtent) / (TWOPI / 4)), rawPath = [], angleIncrement = angleExtent / segments, controlLength = 4 / 3 * _sin2(angleIncrement / 2) / (1 + _cos2(angleIncrement / 2)), ma = cosAngle * rx, mb = sinAngle * rx, mc = sinAngle * -ry, md = cosAngle * ry, i2;
  for (i2 = 0; i2 < segments; i2++) {
    angle = angleStart + i2 * angleIncrement;
    x1 = _cos2(angle);
    y1 = _sin2(angle);
    ux = _cos2(angle += angleIncrement);
    uy = _sin2(angle);
    rawPath.push(x1 - controlLength * y1, y1 + controlLength * x1, ux + controlLength * uy, uy - controlLength * ux, ux, uy);
  }
  for (i2 = 0; i2 < rawPath.length; i2 += 2) {
    x1 = rawPath[i2];
    y1 = rawPath[i2 + 1];
    rawPath[i2] = x1 * ma + y1 * mc + cx;
    rawPath[i2 + 1] = x1 * mb + y1 * md + cy;
  }
  rawPath[i2 - 2] = x;
  rawPath[i2 - 1] = y;
  return rawPath;
}
function stringToRawPath(d) {
  var a = (d + "").replace(_scientific, function(m) {
    var n = +m;
    return n < 1e-4 && n > -1e-4 ? 0 : n;
  }).match(_svgPathExp) || [], path = [], relativeX = 0, relativeY = 0, twoThirds = 2 / 3, elements = a.length, points = 0, errorMessage = "ERROR: malformed path: " + d, i2, j, x, y, command, isRelative, segment, startX, startY, difX, difY, beziers, prevCommand, flag1, flag2, line = function line2(sx, sy, ex, ey) {
    difX = (ex - sx) / 3;
    difY = (ey - sy) / 3;
    segment.push(sx + difX, sy + difY, ex - difX, ey - difY, ex, ey);
  };
  if (!d || !isNaN(a[0]) || isNaN(a[1])) {
    console.log(errorMessage);
    return path;
  }
  for (i2 = 0; i2 < elements; i2++) {
    prevCommand = command;
    if (isNaN(a[i2])) {
      command = a[i2].toUpperCase();
      isRelative = command !== a[i2];
    } else {
      i2--;
    }
    x = +a[i2 + 1];
    y = +a[i2 + 2];
    if (isRelative) {
      x += relativeX;
      y += relativeY;
    }
    if (!i2) {
      startX = x;
      startY = y;
    }
    if (command === "M") {
      if (segment) {
        if (segment.length < 8) {
          path.length -= 1;
        } else {
          points += segment.length;
        }
        _segmentIsClosed(segment);
      }
      relativeX = startX = x;
      relativeY = startY = y;
      segment = [x, y];
      path.push(segment);
      i2 += 2;
      command = "L";
    } else if (command === "C") {
      if (!segment) {
        segment = [0, 0];
      }
      if (!isRelative) {
        relativeX = relativeY = 0;
      }
      segment.push(x, y, relativeX + a[i2 + 3] * 1, relativeY + a[i2 + 4] * 1, relativeX += a[i2 + 5] * 1, relativeY += a[i2 + 6] * 1);
      i2 += 6;
    } else if (command === "S") {
      difX = relativeX;
      difY = relativeY;
      if (prevCommand === "C" || prevCommand === "S") {
        difX += relativeX - segment[segment.length - 4];
        difY += relativeY - segment[segment.length - 3];
      }
      if (!isRelative) {
        relativeX = relativeY = 0;
      }
      segment.push(difX, difY, x, y, relativeX += a[i2 + 3] * 1, relativeY += a[i2 + 4] * 1);
      i2 += 4;
    } else if (command === "Q") {
      difX = relativeX + (x - relativeX) * twoThirds;
      difY = relativeY + (y - relativeY) * twoThirds;
      if (!isRelative) {
        relativeX = relativeY = 0;
      }
      relativeX += a[i2 + 3] * 1;
      relativeY += a[i2 + 4] * 1;
      segment.push(difX, difY, relativeX + (x - relativeX) * twoThirds, relativeY + (y - relativeY) * twoThirds, relativeX, relativeY);
      i2 += 4;
    } else if (command === "T") {
      difX = relativeX - segment[segment.length - 4];
      difY = relativeY - segment[segment.length - 3];
      segment.push(relativeX + difX, relativeY + difY, x + (relativeX + difX * 1.5 - x) * twoThirds, y + (relativeY + difY * 1.5 - y) * twoThirds, relativeX = x, relativeY = y);
      i2 += 2;
    } else if (command === "H") {
      line(relativeX, relativeY, relativeX = x, relativeY);
      i2 += 1;
    } else if (command === "V") {
      line(relativeX, relativeY, relativeX, relativeY = x + (isRelative ? relativeY - relativeX : 0));
      i2 += 1;
    } else if (command === "L" || command === "Z") {
      if (command === "Z") {
        x = startX;
        y = startY;
        segment.closed = true;
      }
      if (command === "L" || _abs(relativeX - x) > 0.5 || _abs(relativeY - y) > 0.5) {
        line(relativeX, relativeY, x, y);
        if (command === "L") {
          i2 += 2;
        }
      }
      relativeX = x;
      relativeY = y;
    } else if (command === "A") {
      flag1 = a[i2 + 4];
      flag2 = a[i2 + 5];
      difX = a[i2 + 6];
      difY = a[i2 + 7];
      j = 7;
      if (flag1.length > 1) {
        if (flag1.length < 3) {
          difY = difX;
          difX = flag2;
          j--;
        } else {
          difY = flag2;
          difX = flag1.substr(2);
          j -= 2;
        }
        flag2 = flag1.charAt(1);
        flag1 = flag1.charAt(0);
      }
      beziers = arcToSegment(relativeX, relativeY, +a[i2 + 1], +a[i2 + 2], +a[i2 + 3], +flag1, +flag2, (isRelative ? relativeX : 0) + difX * 1, (isRelative ? relativeY : 0) + difY * 1);
      i2 += j;
      if (beziers) {
        for (j = 0; j < beziers.length; j++) {
          segment.push(beziers[j]);
        }
      }
      relativeX = segment[segment.length - 2];
      relativeY = segment[segment.length - 1];
    } else {
      console.log(errorMessage);
    }
  }
  i2 = segment.length;
  if (i2 < 6) {
    path.pop();
    i2 = 0;
  } else {
    _segmentIsClosed(segment);
  }
  path.totalPoints = points + i2;
  return path;
}
function bezierToPoints(x1, y1, x2, y2, x3, y3, x4, y4, threshold, points, index) {
  var x12 = (x1 + x2) / 2, y12 = (y1 + y2) / 2, x23 = (x2 + x3) / 2, y23 = (y2 + y3) / 2, x34 = (x3 + x4) / 2, y34 = (y3 + y4) / 2, x123 = (x12 + x23) / 2, y123 = (y12 + y23) / 2, x234 = (x23 + x34) / 2, y234 = (y23 + y34) / 2, x1234 = (x123 + x234) / 2, y1234 = (y123 + y234) / 2, dx = x4 - x1, dy = y4 - y1, d2 = _abs((x2 - x4) * dy - (y2 - y4) * dx), d3 = _abs((x3 - x4) * dy - (y3 - y4) * dx), length;
  if (!points) {
    points = [x1, y1, x4, y4];
    index = 2;
  }
  points.splice(index || points.length - 2, 0, x1234, y1234);
  if ((d2 + d3) * (d2 + d3) > threshold * (dx * dx + dy * dy)) {
    length = points.length;
    bezierToPoints(x1, y1, x12, y12, x123, y123, x1234, y1234, threshold, points, index);
    bezierToPoints(x1234, y1234, x234, y234, x34, y34, x4, y4, threshold, points, index + 2 + (points.length - length));
  }
  return points;
}
function flatPointsToSegment(points, curviness) {
  if (curviness === void 0) {
    curviness = 1;
  }
  var x = points[0], y = 0, segment = [x, y], i2 = 2;
  for (; i2 < points.length; i2 += 2) {
    segment.push(x, y, points[i2], y = (points[i2] - x) * curviness / 2, x = points[i2], -y);
  }
  return segment;
}
function segmentToDistributedPoints(segment, totalPoints) {
  segment.samples || measureSegment(segment);
  var samples = segment.samples, lookup = segment.lookup, resolution = segment.resolution, totalLength = segment.totalLength, points = segment.slice(0, 2), curveStoppingPoints = [], l = segment.length - 4, i2 = 6, limit = 0.2, startLength = 0, curvePointsCumulative = 0, t, curvePoints, min, max, ci, ratioInc, j, inv, curveLength, length, a, nonSmooth, curveStoppingPointIndex, sampleIndex;
  for (; i2 < l; i2 += 6) {
    if (Math.abs(_atan22(segment[i2 + 1] - segment[i2 - 1], segment[i2] - segment[i2 - 2]) - _atan22(segment[i2 + 3] - segment[i2 + 1], segment[i2 + 2] - segment[i2])) > limit) {
      curveStoppingPoints.push(i2);
    }
  }
  curveStoppingPoints.push(segment.length - 2);
  l = curveStoppingPoints.length;
  points.nonSmooth = nonSmooth = [1];
  if (totalPoints > l) {
    totalPoints -= l;
    for (ci = 0; ci < l; ci++) {
      curveStoppingPointIndex = curveStoppingPoints[ci];
      sampleIndex = Math.round(curveStoppingPointIndex / 6 * resolution);
      curveLength = samples[sampleIndex - 1] - startLength;
      curvePoints = Math.round(samples[sampleIndex - 1] / totalLength * totalPoints) - curvePointsCumulative;
      curvePointsCumulative += curvePoints;
      ratioInc = 1 / (curvePoints + 1);
      for (j = 1; j <= curvePoints; j++) {
        length = startLength + curveLength * j * ratioInc;
        i2 = lookup.length ? lookup[length < totalLength ? ~~(length / segment.minLength) : lookup.length - 1] || 0 : _getSampleIndex(samples, length, length / totalLength);
        min = i2 ? samples[i2 - 1] : 0;
        max = samples[i2];
        if (max < length) {
          min = max;
          max = samples[++i2];
        }
        t = 1 / resolution * ((length - min) / (max - min) + i2 % resolution) || 0;
        inv = 1 - t;
        i2 = ~~(i2 / resolution) * 6;
        a = segment[i2];
        points.push(
          _round3((t * t * (segment[i2 + 6] - a) + 3 * inv * (t * (segment[i2 + 4] - a) + inv * (segment[i2 + 2] - a))) * t + a),
          // x
          _round3((t * t * (segment[i2 + 7] - (a = segment[i2 + 1])) + 3 * inv * (t * (segment[i2 + 5] - a) + inv * (segment[i2 + 3] - a))) * t + a)
          // y
        );
      }
      nonSmooth[points.length] = 1;
      points.push(segment[curveStoppingPointIndex], segment[curveStoppingPointIndex + 1]);
      startLength += curveLength;
    }
  }
  i2 = segment.length - 2;
  if (segment.closed && Math.abs(_atan22(segment[i2 + 1] - segment[i2 - 1], segment[i2] - segment[i2 - 2]) - _atan22(segment[3] - segment[1], segment[2] - segment[0])) <= limit) {
    nonSmooth[0] = nonSmooth[nonSmooth.length - 1] = 0;
  }
  return points;
}
function pointsToSegment(points, curviness) {
  _abs(points[0] - points[2]) < 1e-4 && _abs(points[1] - points[3]) < 1e-4 && (points = points.slice(2));
  var l = points.length - 2, x = +points[0], y = +points[1], nextX = +points[2], nextY = +points[3], segment = [x, y, x, y], dx2 = nextX - x, dy2 = nextY - y, nonSmooth = points.nonSmooth || [], closed = Math.abs(points[l] - x) < 1e-3 && Math.abs(points[l + 1] - y) < 1e-3, prevX, prevY, i2, dx1, dy1, r1, r2, r3, tl, mx1, mx2, mxm, my1, my2, mym;
  if (!l) {
    return [x, y, x, y, x, y, x, y];
  }
  if (closed) {
    points.push(nextX, nextY);
    nextX = x;
    nextY = y;
    x = points[l - 2];
    y = points[l - 1];
    points.unshift(x, y);
    l += 4;
    nonSmooth = [0, 0].concat(nonSmooth);
  }
  curviness = curviness || curviness === 0 ? +curviness : 1;
  for (i2 = 2; i2 < l; i2 += 2) {
    prevX = x;
    prevY = y;
    x = nextX;
    y = nextY;
    nextX = +points[i2 + 2];
    nextY = +points[i2 + 3];
    if (x === nextX && y === nextY) {
      continue;
    }
    dx1 = dx2;
    dy1 = dy2;
    dx2 = nextX - x;
    dy2 = nextY - y;
    if (nonSmooth[i2]) {
      segment.push(x - (x - prevX) / 4, y - (y - prevY) / 4, x, y, x + (nextX - x) / 4, y + (nextY - y) / 4);
      continue;
    }
    r1 = _sqrt2(dx1 * dx1 + dy1 * dy1);
    r2 = _sqrt2(dx2 * dx2 + dy2 * dy2);
    r3 = _sqrt2(Math.pow(dx2 / r2 + dx1 / r1, 2) + Math.pow(dy2 / r2 + dy1 / r1, 2));
    tl = (r1 + r2) * curviness * 0.25 / r3;
    mx1 = x - (x - prevX) * (r1 ? tl / r1 : 0);
    mx2 = x + (nextX - x) * (r2 ? tl / r2 : 0);
    mxm = x - (mx1 + ((mx2 - mx1) * (r1 * 3 / (r1 + r2) + 0.5) / 4 || 0));
    my1 = y - (y - prevY) * (r1 ? tl / r1 : 0);
    my2 = y + (nextY - y) * (r2 ? tl / r2 : 0);
    mym = y - (my1 + ((my2 - my1) * (r1 * 3 / (r1 + r2) + 0.5) / 4 || 0));
    segment.push(
      _round3(mx1 + mxm),
      // first control point
      _round3(my1 + mym),
      _round3(x),
      // anchor
      _round3(y),
      _round3(mx2 + mxm),
      // second control point
      _round3(my2 + mym)
    );
  }
  x !== nextX || y !== nextY || segment.length < 4 ? segment.push(_round3(nextX), _round3(nextY), _round3(nextX), _round3(nextY)) : segment.length -= 2;
  if (segment.length === 2) {
    segment.push(x, y, x, y, x, y);
  } else if (closed) {
    segment.splice(0, 6);
    segment.length -= 6;
  }
  segment.closed = closed;
  return segment;
}
function pointToSegDist(x, y, x1, y1, x2, y2) {
  var dx = x2 - x1, dy = y2 - y1, t;
  if (dx || dy) {
    t = ((x - x1) * dx + (y - y1) * dy) / (dx * dx + dy * dy);
    if (t > 1) {
      x1 = x2;
      y1 = y2;
    } else if (t > 0) {
      x1 += dx * t;
      y1 += dy * t;
    }
  }
  return Math.pow(x - x1, 2) + Math.pow(y - y1, 2);
}
function simplifyStep(points, first, last, tolerance, simplified) {
  var maxSqDist = tolerance, firstX = points[first], firstY = points[first + 1], lastX = points[last], lastY = points[last + 1], index, i2, d;
  for (i2 = first + 2; i2 < last; i2 += 2) {
    d = pointToSegDist(points[i2], points[i2 + 1], firstX, firstY, lastX, lastY);
    if (d > maxSqDist) {
      index = i2;
      maxSqDist = d;
    }
  }
  if (maxSqDist > tolerance) {
    index - first > 2 && simplifyStep(points, first, index, tolerance, simplified);
    simplified.push(points[index], points[index + 1]);
    last - index > 2 && simplifyStep(points, index, last, tolerance, simplified);
  }
}
function simplifyPoints(points, tolerance) {
  var prevX = parseFloat(points[0]), prevY = parseFloat(points[1]), temp = [prevX, prevY], l = points.length - 2, i2, x, y, dx, dy, result, last;
  tolerance = Math.pow(tolerance || 1, 2);
  for (i2 = 2; i2 < l; i2 += 2) {
    x = parseFloat(points[i2]);
    y = parseFloat(points[i2 + 1]);
    dx = prevX - x;
    dy = prevY - y;
    if (dx * dx + dy * dy > tolerance) {
      temp.push(x, y);
      prevX = x;
      prevY = y;
    }
  }
  temp.push(parseFloat(points[l]), parseFloat(points[l + 1]));
  last = temp.length - 2;
  result = [temp[0], temp[1]];
  simplifyStep(temp, 0, last, tolerance, result);
  result.push(temp[last], temp[last + 1]);
  return result;
}
function getClosestProgressOnBezier(iterations, px, py, start, end, slices, x0, y0, x1, y1, x2, y2, x3, y3) {
  var inc = (end - start) / slices, best = 0, t = start, x, y, d, dx, dy, inv;
  _bestDistance = _largeNum;
  while (t <= end) {
    inv = 1 - t;
    x = inv * inv * inv * x0 + 3 * inv * inv * t * x1 + 3 * inv * t * t * x2 + t * t * t * x3;
    y = inv * inv * inv * y0 + 3 * inv * inv * t * y1 + 3 * inv * t * t * y2 + t * t * t * y3;
    dx = x - px;
    dy = y - py;
    d = dx * dx + dy * dy;
    if (d < _bestDistance) {
      _bestDistance = d;
      best = t;
    }
    t += inc;
  }
  return iterations > 1 ? getClosestProgressOnBezier(iterations - 1, px, py, Math.max(best - inc, 0), Math.min(best + inc, 1), slices, x0, y0, x1, y1, x2, y2, x3, y3) : best;
}
function getClosestData(rawPath, x, y, slices) {
  var closest = {
    j: 0,
    i: 0,
    t: 0
  }, bestDistance = _largeNum, i2, j, t, segment;
  for (j = 0; j < rawPath.length; j++) {
    segment = rawPath[j];
    for (i2 = 0; i2 < segment.length; i2 += 6) {
      t = getClosestProgressOnBezier(1, x, y, 0, 1, slices || 20, segment[i2], segment[i2 + 1], segment[i2 + 2], segment[i2 + 3], segment[i2 + 4], segment[i2 + 5], segment[i2 + 6], segment[i2 + 7]);
      if (bestDistance > _bestDistance) {
        bestDistance = _bestDistance;
        closest.j = j;
        closest.i = i2;
        closest.t = t;
      }
    }
  }
  return closest;
}
function rawPathToString(rawPath) {
  if (_isNumber3(rawPath[0])) {
    rawPath = [rawPath];
  }
  var result = "", l = rawPath.length, sl, s, i2, segment;
  for (s = 0; s < l; s++) {
    segment = rawPath[s];
    result += "M" + _round3(segment[0]) + "," + _round3(segment[1]) + " C";
    sl = segment.length;
    for (i2 = 2; i2 < sl; i2++) {
      result += _round3(segment[i2++]) + "," + _round3(segment[i2++]) + " " + _round3(segment[i2++]) + "," + _round3(segment[i2++]) + " " + _round3(segment[i2++]) + "," + _round3(segment[i2]) + " ";
    }
    if (segment.closed) {
      result += "z";
    }
  }
  return result;
}

// examples/figma-launch/node_modules/gsap/CustomEase.js
var gsap2;
var _coreInitted2;
var _getGSAP = function _getGSAP2() {
  return gsap2 || typeof window !== "undefined" && (gsap2 = window.gsap) && gsap2.registerPlugin && gsap2;
};
var _initCore3 = function _initCore4() {
  gsap2 = _getGSAP();
  if (gsap2) {
    gsap2.registerEase("_CE", CustomEase.create);
    _coreInitted2 = 1;
  } else {
    console.warn("Please gsap.registerPlugin(CustomEase)");
  }
};
var _bigNum3 = 1e20;
var _round5 = function _round6(value) {
  return ~~(value * 1e3 + (value < 0 ? -0.5 : 0.5)) / 1e3;
};
var _bonusValidated = 1;
var _numExp2 = /[-+=.]*\d+[.e\-+]*\d*[e\-+]*\d*/gi;
var _needsParsingExp = /[cLlsSaAhHvVtTqQ]/g;
var _findMinimum = function _findMinimum2(values) {
  var l = values.length, min = _bigNum3, i2;
  for (i2 = 1; i2 < l; i2 += 6) {
    +values[i2] < min && (min = +values[i2]);
  }
  return min;
};
var _normalize = function _normalize2(values, height, originY) {
  if (!originY && originY !== 0) {
    originY = Math.max(+values[values.length - 1], +values[1]);
  }
  var tx = +values[0] * -1, ty = -originY, l = values.length, sx = 1 / (+values[l - 2] + tx), sy = -height || (Math.abs(+values[l - 1] - +values[1]) < 0.01 * (+values[l - 2] - +values[0]) ? _findMinimum(values) + ty : +values[l - 1] + ty), i2;
  if (sy) {
    sy = 1 / sy;
  } else {
    sy = -sx;
  }
  for (i2 = 0; i2 < l; i2 += 2) {
    values[i2] = (+values[i2] + tx) * sx;
    values[i2 + 1] = (+values[i2 + 1] + ty) * sy;
  }
};
var _bezierToPoints = function _bezierToPoints2(x1, y1, x2, y2, x3, y3, x4, y4, threshold, points, index) {
  var x12 = (x1 + x2) / 2, y12 = (y1 + y2) / 2, x23 = (x2 + x3) / 2, y23 = (y2 + y3) / 2, x34 = (x3 + x4) / 2, y34 = (y3 + y4) / 2, x123 = (x12 + x23) / 2, y123 = (y12 + y23) / 2, x234 = (x23 + x34) / 2, y234 = (y23 + y34) / 2, x1234 = (x123 + x234) / 2, y1234 = (y123 + y234) / 2, dx = x4 - x1, dy = y4 - y1, d2 = Math.abs((x2 - x4) * dy - (y2 - y4) * dx), d3 = Math.abs((x3 - x4) * dy - (y3 - y4) * dx), length;
  if (!points) {
    points = [{
      x: x1,
      y: y1
    }, {
      x: x4,
      y: y4
    }];
    index = 1;
  }
  points.splice(index || points.length - 1, 0, {
    x: x1234,
    y: y1234
  });
  if ((d2 + d3) * (d2 + d3) > threshold * (dx * dx + dy * dy)) {
    length = points.length;
    _bezierToPoints2(x1, y1, x12, y12, x123, y123, x1234, y1234, threshold, points, index);
    _bezierToPoints2(x1234, y1234, x234, y234, x34, y34, x4, y4, threshold, points, index + 1 + (points.length - length));
  }
  return points;
};
var CustomEase = /* @__PURE__ */ (function() {
  function CustomEase2(id, data, config3) {
    _coreInitted2 || _initCore3();
    this.id = id;
    _bonusValidated && this.setData(data, config3);
  }
  var _proto = CustomEase2.prototype;
  _proto.setData = function setData(data, config3) {
    config3 = config3 || {};
    data = data || "0,0,1,1";
    var values = data.match(_numExp2), closest = 1, points = [], lookup = [], precision = config3.precision || 1, fast = precision <= 1, l, a1, a2, i2, inc, j, point, prevPoint, p2;
    this.data = data;
    if (_needsParsingExp.test(data) || ~data.indexOf("M") && data.indexOf("C") < 0) {
      values = stringToRawPath(data)[0];
    }
    l = values.length;
    if (l === 4) {
      values.unshift(0, 0);
      values.push(1, 1);
      l = 8;
    } else if ((l - 2) % 6) {
      throw "Invalid CustomEase";
    }
    if (+values[0] !== 0 || +values[l - 2] !== 1) {
      _normalize(values, config3.height, config3.originY);
    }
    this.segment = values;
    for (i2 = 2; i2 < l; i2 += 6) {
      a1 = {
        x: +values[i2 - 2],
        y: +values[i2 - 1]
      };
      a2 = {
        x: +values[i2 + 4],
        y: +values[i2 + 5]
      };
      points.push(a1, a2);
      _bezierToPoints(a1.x, a1.y, +values[i2], +values[i2 + 1], +values[i2 + 2], +values[i2 + 3], a2.x, a2.y, 1 / (precision * 2e5), points, points.length - 1);
    }
    l = points.length;
    for (i2 = 0; i2 < l; i2++) {
      point = points[i2];
      prevPoint = points[i2 - 1] || point;
      if ((point.x > prevPoint.x || prevPoint.y !== point.y && prevPoint.x === point.x || point === prevPoint) && point.x <= 1) {
        prevPoint.cx = point.x - prevPoint.x;
        prevPoint.cy = point.y - prevPoint.y;
        prevPoint.n = point;
        prevPoint.nx = point.x;
        if (fast && i2 > 1 && Math.abs(prevPoint.cy / prevPoint.cx - points[i2 - 2].cy / points[i2 - 2].cx) > 2) {
          fast = 0;
        }
        if (prevPoint.cx < closest) {
          if (!prevPoint.cx) {
            prevPoint.cx = 1e-3;
            if (i2 === l - 1) {
              prevPoint.x -= 1e-3;
              closest = Math.min(closest, 1e-3);
              fast = 0;
            }
          } else {
            closest = prevPoint.cx;
          }
        }
      } else {
        points.splice(i2--, 1);
        l--;
      }
    }
    l = 1 / closest + 1 | 0;
    inc = 1 / l;
    j = 0;
    point = points[0];
    if (fast) {
      for (i2 = 0; i2 < l; i2++) {
        p2 = i2 * inc;
        if (point.nx < p2) {
          point = points[++j];
        }
        a1 = point.y + (p2 - point.x) / point.cx * point.cy;
        lookup[i2] = {
          x: p2,
          cx: inc,
          y: a1,
          cy: 0,
          nx: 9
        };
        if (i2) {
          lookup[i2 - 1].cy = a1 - lookup[i2 - 1].y;
        }
      }
      j = points[points.length - 1];
      lookup[l - 1].cy = j.y - a1;
      lookup[l - 1].cx = j.x - lookup[lookup.length - 1].x;
    } else {
      for (i2 = 0; i2 < l; i2++) {
        if (point.nx < i2 * inc) {
          point = points[++j];
        }
        lookup[i2] = point;
      }
      if (j < points.length - 1) {
        lookup[i2 - 1] = points[points.length - 2];
      }
    }
    this.ease = function(p3) {
      var point2 = lookup[p3 * l | 0] || lookup[l - 1];
      if (point2.nx < p3) {
        point2 = point2.n;
      }
      return point2.y + (p3 - point2.x) / point2.cx * point2.cy;
    };
    this.ease.custom = this;
    this.id && gsap2 && gsap2.registerEase(this.id, this.ease);
    return this;
  };
  _proto.getSVGData = function getSVGData(config3) {
    return CustomEase2.getSVGData(this, config3);
  };
  CustomEase2.create = function create(id, data, config3) {
    return new CustomEase2(id, data, config3).ease;
  };
  CustomEase2.register = function register8(core) {
    gsap2 = core;
    _initCore3();
  };
  CustomEase2.get = function get(id) {
    return gsap2.parseEase(id);
  };
  CustomEase2.getSVGData = function getSVGData(ease, config3) {
    config3 = config3 || {};
    var width = config3.width || 100, height = config3.height || 100, x = config3.x || 0, y = (config3.y || 0) + height, e = gsap2.utils.toArray(config3.path)[0], a, slope, i2, inc, tx, ty, precision, threshold, prevX, prevY;
    if (config3.invert) {
      height = -height;
      y = 0;
    }
    if (typeof ease === "string") {
      ease = gsap2.parseEase(ease);
    }
    if (ease.custom) {
      ease = ease.custom;
    }
    if (ease instanceof CustomEase2) {
      a = rawPathToString(transformRawPath([ease.segment.slice(0)], width, 0, 0, -height, x, y));
    } else {
      a = [x, y];
      precision = Math.max(5, (config3.precision || 1) * 200);
      inc = 1 / precision;
      precision += 2;
      threshold = 5 / precision;
      prevX = _round5(x + inc * width);
      prevY = _round5(y + ease(inc) * -height);
      slope = (prevY - y) / (prevX - x);
      for (i2 = 2; i2 < precision; i2++) {
        tx = _round5(x + i2 * inc * width);
        ty = _round5(y + ease(i2 * inc) * -height);
        if (Math.abs((ty - prevY) / (tx - prevX) - slope) > threshold || i2 === precision - 1) {
          a.push(prevX, prevY);
          slope = (ty - prevY) / (tx - prevX);
        }
        prevX = tx;
        prevY = ty;
      }
      a = "M" + a.join(",");
    }
    e && e.setAttribute("d", a);
    return a;
  };
  return CustomEase2;
})();
CustomEase.version = "3.15.0";
CustomEase.headless = true;
_getGSAP() && gsap2.registerPlugin(CustomEase);

// examples/figma-launch/node_modules/gsap/CustomWiggle.js
var gsap3;
var _coreInitted3;
var createCustomEase;
var _getGSAP3 = function _getGSAP4() {
  return gsap3 || typeof window !== "undefined" && (gsap3 = window.gsap) && gsap3.registerPlugin && gsap3;
};
var _eases = {
  easeOut: "M0,1,C0.7,1,0.6,0,1,0",
  easeInOut: "M0,0,C0.1,0,0.24,1,0.444,1,0.644,1,0.6,0,1,0",
  anticipate: "M0,0,C0,0.222,0.024,0.386,0,0.4,0.18,0.455,0.65,0.646,0.7,0.67,0.9,0.76,1,0.846,1,1",
  uniform: "M0,0,C0,0.95,0,1,0,1,0,1,1,1,1,1,1,1,1,0,1,0"
};
var _linearEase = function _linearEase2(p2) {
  return p2;
};
var _initCore5 = function _initCore6(required) {
  if (!_coreInitted3) {
    gsap3 = _getGSAP3();
    createCustomEase = gsap3 && gsap3.parseEase("_CE");
    if (createCustomEase) {
      for (var p2 in _eases) {
        _eases[p2] = createCustomEase("", _eases[p2]);
      }
      _coreInitted3 = 1;
      _create("wiggle").config = function(vars) {
        return typeof vars === "object" ? _create("", vars) : _create("wiggle(" + vars + ")", {
          wiggles: +vars
        });
      };
    } else {
      required && console.warn("Please gsap.registerPlugin(CustomEase, CustomWiggle)");
    }
  }
};
var _parseEase3 = function _parseEase4(ease, invertNonCustomEases) {
  if (typeof ease !== "function") {
    ease = gsap3.parseEase(ease) || createCustomEase("", ease);
  }
  return ease.custom || !invertNonCustomEases ? ease : function(p2) {
    return 1 - ease(p2);
  };
};
var _bonusValidated2 = 1;
var _create = function _create2(id, vars) {
  if (!_coreInitted3) {
    _initCore5(1);
  }
  vars = vars || {};
  var wiggles = (vars.wiggles || 10) | 0, inc = 1 / wiggles, x = inc / 2, anticipate = vars.type === "anticipate", yEase = _eases[vars.type] || _eases.easeOut, xEase = _linearEase, rnd = 1e3, nextX, nextY, angle, handleX, handleY, easedX, y, path, i2;
  if (_bonusValidated2) {
    if (anticipate) {
      xEase = yEase;
      yEase = _eases.easeOut;
    }
    if (vars.timingEase) {
      xEase = _parseEase3(vars.timingEase);
    }
    if (vars.amplitudeEase) {
      yEase = _parseEase3(vars.amplitudeEase, true);
    }
    easedX = xEase(x);
    y = anticipate ? -yEase(x) : yEase(x);
    path = [0, 0, easedX / 4, 0, easedX / 2, y, easedX, y];
    if (vars.type === "random") {
      path.length = 4;
      nextX = xEase(inc);
      nextY = Math.random() * 2 - 1;
      for (i2 = 2; i2 < wiggles; i2++) {
        x = nextX;
        y = nextY;
        nextX = xEase(inc * i2);
        nextY = Math.random() * 2 - 1;
        angle = Math.atan2(nextY - path[path.length - 3], nextX - path[path.length - 4]);
        handleX = Math.cos(angle) * inc;
        handleY = Math.sin(angle) * inc;
        path.push(x - handleX, y - handleY, x, y, x + handleX, y + handleY);
      }
      path.push(nextX, 0, 1, 0);
    } else {
      for (i2 = 1; i2 < wiggles; i2++) {
        path.push(xEase(x + inc / 2), y);
        x += inc;
        y = (y > 0 ? -1 : 1) * yEase(i2 * inc);
        easedX = xEase(x);
        path.push(xEase(x - inc / 2), y, easedX, y);
      }
      path.push(xEase(x + inc / 4), y, xEase(x + inc / 4), 0, 1, 0);
    }
    i2 = path.length;
    while (--i2 > -1) {
      path[i2] = ~~(path[i2] * rnd) / rnd;
    }
    path[2] = "C" + path[2];
    return createCustomEase(id, "M" + path.join(","));
  }
};
var CustomWiggle = /* @__PURE__ */ (function() {
  function CustomWiggle2(id, vars) {
    this.ease = _create(id, vars);
  }
  CustomWiggle2.create = function create(id, vars) {
    return _create(id, vars);
  };
  CustomWiggle2.register = function register8(core) {
    gsap3 = core;
    _initCore5();
  };
  return CustomWiggle2;
})();
_getGSAP3() && gsap3.registerPlugin(CustomWiggle);
CustomWiggle.version = "3.15.0";

// examples/figma-launch/node_modules/gsap/CustomBounce.js
var gsap4;
var _coreInitted4;
var createCustomEase2;
var _getGSAP5 = function _getGSAP6() {
  return gsap4 || typeof window !== "undefined" && (gsap4 = window.gsap) && gsap4.registerPlugin && gsap4;
};
var _initCore7 = function _initCore8(required) {
  gsap4 = _getGSAP5();
  createCustomEase2 = gsap4 && gsap4.parseEase("_CE");
  if (createCustomEase2) {
    _coreInitted4 = 1;
    gsap4.parseEase("bounce").config = function(vars) {
      return typeof vars === "object" ? _create3("", vars) : _create3("bounce(" + vars + ")", {
        strength: +vars
      });
    };
  } else {
    required && console.warn("Please gsap.registerPlugin(CustomEase, CustomBounce)");
  }
};
var _normalizeX = function _normalizeX2(a) {
  var l = a.length, s = 1 / a[l - 2], rnd = 1e3, i2;
  for (i2 = 2; i2 < l; i2 += 2) {
    a[i2] = ~~(a[i2] * s * rnd) / rnd;
  }
  a[l - 2] = 1;
};
var _bonusValidated3 = 1;
var _create3 = function _create4(id, vars) {
  if (!_coreInitted4) {
    _initCore7(1);
  }
  vars = vars || {};
  if (_bonusValidated3) {
    var max = 0.999, decay = Math.min(max, vars.strength || 0.7), decayX = decay, gap = (vars.squash || 0) / 100, originalGap = gap, slope = 1 / 0.03, w = 0.2, h = 1, prevX = 0.1, path = [0, 0, 0.07, 0, 0.1, 1, 0.1, 1], squashPath = [0, 0, 0, 0, 0.1, 0, 0.1, 0], cp1, cp2, x, y, i2, nextX, squishMagnitude;
    for (i2 = 0; i2 < 200; i2++) {
      w *= decayX * ((decayX + 1) / 2);
      h *= decay * decay;
      nextX = prevX + w;
      x = prevX + w * 0.49;
      y = 1 - h;
      cp1 = prevX + h / slope;
      cp2 = x + (x - cp1) * 0.8;
      if (gap) {
        prevX += gap;
        cp1 += gap;
        x += gap;
        cp2 += gap;
        nextX += gap;
        squishMagnitude = gap / originalGap;
        squashPath.push(
          prevX - gap,
          0,
          prevX - gap,
          squishMagnitude,
          prevX - gap / 2,
          squishMagnitude,
          //center peak anchor
          prevX,
          squishMagnitude,
          prevX,
          0,
          prevX,
          0,
          //base anchor
          prevX,
          squishMagnitude * -0.6,
          prevX + (nextX - prevX) / 6,
          0,
          nextX,
          0
        );
        path.push(prevX - gap, 1, prevX, 1, prevX, 1);
        gap *= decay * decay;
      }
      path.push(prevX, 1, cp1, y, x, y, cp2, y, nextX, 1, nextX, 1);
      decay *= 0.95;
      slope = h / (nextX - cp2);
      prevX = nextX;
      if (y > max) {
        break;
      }
    }
    if (vars.endAtStart && vars.endAtStart !== "false") {
      x = -0.1;
      path.unshift(x, 1, x, 1, -0.07, 0);
      if (originalGap) {
        gap = originalGap * 2.5;
        x -= gap;
        path.unshift(x, 1, x, 1, x, 1);
        squashPath.splice(0, 6);
        squashPath.unshift(x, 0, x, 0, x, 1, x + gap / 2, 1, x + gap, 1, x + gap, 0, x + gap, 0, x + gap, -0.6, x + gap + 0.033, 0);
        for (i2 = 0; i2 < squashPath.length; i2 += 2) {
          squashPath[i2] -= x;
        }
      }
      for (i2 = 0; i2 < path.length; i2 += 2) {
        path[i2] -= x;
        path[i2 + 1] = 1 - path[i2 + 1];
      }
    }
    if (gap) {
      _normalizeX(squashPath);
      squashPath[2] = "C" + squashPath[2];
      createCustomEase2(vars.squashID || id + "-squash", "M" + squashPath.join(","));
    }
    _normalizeX(path);
    path[2] = "C" + path[2];
    return createCustomEase2(id, "M" + path.join(","));
  }
};
var CustomBounce = /* @__PURE__ */ (function() {
  function CustomBounce2(id, vars) {
    this.ease = _create3(id, vars);
  }
  CustomBounce2.create = function create(id, vars) {
    return _create3(id, vars);
  };
  CustomBounce2.register = function register8(core) {
    gsap4 = core;
    _initCore7();
  };
  return CustomBounce2;
})();
_getGSAP5() && gsap4.registerPlugin(CustomBounce);
CustomBounce.version = "3.15.0";

// examples/figma-launch/node_modules/gsap/EasePack.js
var gsap5;
var _coreInitted5;
var _registerEase;
var _getGSAP7 = function _getGSAP8() {
  return gsap5 || typeof window !== "undefined" && (gsap5 = window.gsap) && gsap5.registerPlugin && gsap5;
};
var _boolean = function _boolean2(value, defaultValue) {
  return !!(typeof value === "undefined" ? defaultValue : value && !~(value + "").indexOf("false"));
};
var _initCore9 = function _initCore10(core) {
  gsap5 = core || _getGSAP7();
  if (gsap5) {
    _registerEase = gsap5.registerEase;
    var eases = gsap5.parseEase(), createConfig = function createConfig2(ease) {
      return function(ratio) {
        var y = 0.5 + ratio / 2;
        ease.config = function(p3) {
          return ease(2 * (1 - p3) * p3 * y + p3 * p3);
        };
      };
    }, p2;
    for (p2 in eases) {
      if (!eases[p2].config) {
        createConfig(eases[p2]);
      }
    }
    _registerEase("slow", SlowMo);
    _registerEase("expoScale", ExpoScaleEase);
    _registerEase("rough", RoughEase);
    for (p2 in EasePack) {
      p2 !== "version" && gsap5.core.globals(p2, EasePack[p2]);
    }
    _coreInitted5 = 1;
  }
};
var _createSlowMo = function _createSlowMo2(linearRatio, power, yoyoMode) {
  linearRatio = Math.min(1, linearRatio || 0.7);
  var pow = linearRatio < 1 ? power || power === 0 ? power : 0.7 : 0, p1 = (1 - linearRatio) / 2, p3 = p1 + linearRatio, calcEnd = _boolean(yoyoMode);
  return function(p2) {
    var r = p2 + (0.5 - p2) * pow;
    return p2 < p1 ? calcEnd ? 1 - (p2 = 1 - p2 / p1) * p2 : r - (p2 = 1 - p2 / p1) * p2 * p2 * p2 * r : p2 > p3 ? calcEnd ? p2 === 1 ? 0 : 1 - (p2 = (p2 - p3) / p1) * p2 : r + (p2 - r) * (p2 = (p2 - p3) / p1) * p2 * p2 * p2 : calcEnd ? 1 : r;
  };
};
var _createExpoScale = function _createExpoScale2(start, end, ease) {
  var p1 = Math.log(end / start), p2 = end - start;
  ease && (ease = gsap5.parseEase(ease));
  return function(p3) {
    return (start * Math.exp(p1 * (ease ? ease(p3) : p3)) - start) / p2;
  };
};
var EasePoint = function EasePoint2(time, value, next) {
  this.t = time;
  this.v = value;
  if (next) {
    this.next = next;
    next.prev = this;
    this.c = next.v - value;
    this.gap = next.t - time;
  }
};
var _createRoughEase = function _createRoughEase2(vars) {
  if (typeof vars !== "object") {
    vars = {
      points: +vars || 20
    };
  }
  var taper = vars.taper || "none", a = [], cnt = 0, points = (+vars.points || 20) | 0, i2 = points, randomize = _boolean(vars.randomize, true), clamp3 = _boolean(vars.clamp), template = gsap5 ? gsap5.parseEase(vars.template) : 0, strength = (+vars.strength || 1) * 0.4, x, y, bump, invX, obj, pnt, recent;
  while (--i2 > -1) {
    x = randomize ? Math.random() : 1 / points * i2;
    y = template ? template(x) : x;
    if (taper === "none") {
      bump = strength;
    } else if (taper === "out") {
      invX = 1 - x;
      bump = invX * invX * strength;
    } else if (taper === "in") {
      bump = x * x * strength;
    } else if (x < 0.5) {
      invX = x * 2;
      bump = invX * invX * 0.5 * strength;
    } else {
      invX = (1 - x) * 2;
      bump = invX * invX * 0.5 * strength;
    }
    if (randomize) {
      y += Math.random() * bump - bump * 0.5;
    } else if (i2 % 2) {
      y += bump * 0.5;
    } else {
      y -= bump * 0.5;
    }
    if (clamp3) {
      if (y > 1) {
        y = 1;
      } else if (y < 0) {
        y = 0;
      }
    }
    a[cnt++] = {
      x,
      y
    };
  }
  a.sort(function(a2, b) {
    return a2.x - b.x;
  });
  pnt = new EasePoint(1, 1, null);
  i2 = points;
  while (i2--) {
    obj = a[i2];
    pnt = new EasePoint(obj.x, obj.y, pnt);
  }
  recent = new EasePoint(0, 0, pnt.t ? pnt : pnt.next);
  return function(p2) {
    var pnt2 = recent;
    if (p2 > pnt2.t) {
      while (pnt2.next && p2 >= pnt2.t) {
        pnt2 = pnt2.next;
      }
      pnt2 = pnt2.prev;
    } else {
      while (pnt2.prev && p2 <= pnt2.t) {
        pnt2 = pnt2.prev;
      }
    }
    recent = pnt2;
    return pnt2.v + (p2 - pnt2.t) / pnt2.gap * pnt2.c;
  };
};
var SlowMo = _createSlowMo(0.7);
SlowMo.ease = SlowMo;
SlowMo.config = _createSlowMo;
var ExpoScaleEase = _createExpoScale(1, 2);
ExpoScaleEase.config = _createExpoScale;
var RoughEase = _createRoughEase();
RoughEase.ease = RoughEase;
RoughEase.config = _createRoughEase;
var EasePack = {
  SlowMo,
  RoughEase,
  ExpoScaleEase
};
for (p2 in EasePack) {
  EasePack[p2].register = _initCore9;
  EasePack[p2].version = "3.15.0";
}
var p2;
_getGSAP7() && gsap5.registerPlugin(SlowMo);

// examples/figma-launch/node_modules/gsap/SplitText.js
var gsap6;
var _fonts;
var _splitProp = typeof Symbol === "function" ? Symbol() : "_split";
var _coreInitted6;
var _initIfNecessary = () => _coreInitted6 || SplitText.register(window.gsap);
var _charSegmenter = typeof Intl !== "undefined" && "Segmenter" in Intl ? new Intl.Segmenter() : 0;
var _toArray = (r) => !r ? [] : typeof r === "string" ? _toArray(document.querySelectorAll(r)) : "length" in r ? Array.from(r).reduce((acc, cur) => {
  typeof cur === "string" ? acc.push(..._toArray(cur)) : acc.push(cur);
  return acc;
}, []) : [r];
var _elements = (targets) => _toArray(targets).filter((e) => e && e.nodeType === 1);
var _emptyArray2 = [];
var _context2 = function() {
};
var _defaultContext = { add: (f) => f() };
var _spacesRegEx = /\s+/g;
var _emojiSafeRegEx = new RegExp("\\p{RI}\\p{RI}|\\p{Emoji}(\\p{EMod}|\\u{FE0F}\\u{20E3}?|[\\u{E0020}-\\u{E007E}]+\\u{E007F})?(\\u{200D}\\p{Emoji}(\\p{EMod}|\\u{FE0F}\\u{20E3}?|[\\u{E0020}-\\u{E007E}]+\\u{E007F})?)*|.", "gu");
var _emptyBounds = { left: 0, top: 0, width: 0, height: 0 };
var _findNextValidBounds = (allBounds, startIndex) => {
  while (++startIndex < allBounds.length && allBounds[startIndex] === _emptyBounds) {
  }
  return allBounds[startIndex] || _emptyBounds;
};
var _revertOriginal = ({ element, html, ariaL, ariaH }) => {
  element.innerHTML = html;
  ariaL ? element.setAttribute("aria-label", ariaL) : element.removeAttribute("aria-label");
  ariaH ? element.setAttribute("aria-hidden", ariaH) : element.removeAttribute("aria-hidden");
};
var _stretchToFitSpecialChars = (collection, specialCharsRegEx) => {
  if (specialCharsRegEx) {
    let charsFound = new Set(collection.join("").match(specialCharsRegEx) || _emptyArray2), i2 = collection.length, slots, word, char, combined;
    if (charsFound.size) {
      while (--i2 > -1) {
        word = collection[i2];
        for (char of charsFound) {
          if (char.startsWith(word) && char.length > word.length) {
            slots = 0;
            combined = word;
            while (char.startsWith(combined += collection[i2 + ++slots]) && combined.length < char.length) {
            }
            if (slots && combined.length === char.length) {
              collection[i2] = char;
              collection.splice(i2 + 1, slots);
              break;
            }
          }
        }
      }
    }
  }
  return collection;
};
var _disallowInline = (element) => window.getComputedStyle(element).display === "inline" && (element.style.display = "inline-block");
var _insertNodeBefore = (newChild, parent, existingChild) => parent.insertBefore(typeof newChild === "string" ? document.createTextNode(newChild) : newChild, existingChild);
var _getWrapper = (type, config3, collection) => {
  let className = config3[type + "sClass"] || "", { tag = "div", aria = "auto", propIndex = false } = config3, display = type === "line" ? "block" : "inline-block", incrementClass = className.indexOf("++") > -1, wrapper = (text) => {
    let el = document.createElement(tag), i2 = collection.length + 1;
    className && (el.className = className + (incrementClass ? " " + className + i2 : ""));
    propIndex && el.style.setProperty("--" + type, i2 + "");
    aria !== "none" && el.setAttribute("aria-hidden", "true");
    if (tag !== "span") {
      el.style.position = "relative";
      el.style.display = display;
    }
    el.textContent = text;
    collection.push(el);
    return el;
  };
  incrementClass && (className = className.replace("++", ""));
  wrapper.collection = collection;
  return wrapper;
};
var _getLineWrapper = (element, nodes, config3, collection) => {
  let lineWrapper = _getWrapper("line", config3, collection), textAlign = window.getComputedStyle(element).textAlign || "left";
  return (startIndex, endIndex) => {
    let newLine = lineWrapper("");
    newLine.style.textAlign = textAlign;
    element.insertBefore(newLine, nodes[startIndex]);
    for (; startIndex < endIndex; startIndex++) {
      newLine.appendChild(nodes[startIndex]);
    }
    newLine.normalize();
  };
};
var _splitWordsAndCharsRecursively = (element, config3, wordWrapper, charWrapper, prepForCharsOnly, deepSlice, ignore, charSplitRegEx, specialCharsRegEx, isNested) => {
  var _a;
  let nodes = Array.from(element.childNodes), i2 = 0, { wordDelimiter, reduceWhiteSpace = true, prepareText } = config3, elementBounds = element.getBoundingClientRect(), lastBounds = elementBounds, isPreformatted = !reduceWhiteSpace && window.getComputedStyle(element).whiteSpace.substring(0, 3) === "pre", ignoredPreviousSibling = 0, wordsCollection = wordWrapper.collection, wordDelimIsNotSpace, wordDelimString, wordDelimSplitter, curNode, words, curWordEl, startsWithSpace, endsWithSpace, j, bounds, curWordChars, clonedNode, curSubNode, tempSubNode, curTextContent, wordText, lastWordText, k;
  if (typeof wordDelimiter === "object") {
    wordDelimSplitter = wordDelimiter.delimiter || wordDelimiter;
    wordDelimString = wordDelimiter.replaceWith || "";
  } else {
    wordDelimString = wordDelimiter === "" ? "" : wordDelimiter || " ";
  }
  wordDelimIsNotSpace = wordDelimString !== " ";
  for (; i2 < nodes.length; i2++) {
    curNode = nodes[i2];
    if (curNode.nodeType === 3) {
      curTextContent = curNode.textContent || "";
      if (reduceWhiteSpace) {
        curTextContent = curTextContent.replace(_spacesRegEx, " ");
      } else if (isPreformatted) {
        curTextContent = curTextContent.replace(/\n/g, wordDelimString + "\n");
      }
      prepareText && (curTextContent = prepareText(curTextContent, element));
      curNode.textContent = curTextContent;
      words = wordDelimString || wordDelimSplitter ? curTextContent.split(wordDelimSplitter || wordDelimString) : curTextContent.match(charSplitRegEx) || _emptyArray2;
      lastWordText = words[words.length - 1];
      endsWithSpace = wordDelimIsNotSpace ? lastWordText.slice(-1) === " " : !lastWordText;
      lastWordText || words.pop();
      lastBounds = elementBounds;
      startsWithSpace = wordDelimIsNotSpace ? words[0].charAt(0) === " " : !words[0];
      startsWithSpace && _insertNodeBefore(" ", element, curNode);
      words[0] || words.shift();
      _stretchToFitSpecialChars(words, specialCharsRegEx);
      deepSlice && isNested || (curNode.textContent = "");
      for (j = 1; j <= words.length; j++) {
        wordText = words[j - 1];
        if (!reduceWhiteSpace && isPreformatted && wordText.charAt(0) === "\n") {
          (_a = curNode.previousSibling) == null ? void 0 : _a.remove();
          _insertNodeBefore(document.createElement("br"), element, curNode);
          wordText = wordText.slice(1);
        }
        if (!reduceWhiteSpace && wordText === "") {
          _insertNodeBefore(wordDelimString, element, curNode);
        } else if (wordText === " ") {
          element.insertBefore(document.createTextNode(" "), curNode);
        } else {
          wordDelimIsNotSpace && wordText.charAt(0) === " " && _insertNodeBefore(" ", element, curNode);
          if (ignoredPreviousSibling && j === 1 && !startsWithSpace && wordsCollection.indexOf(ignoredPreviousSibling.parentNode) > -1) {
            curWordEl = wordsCollection[wordsCollection.length - 1];
            curWordEl.appendChild(document.createTextNode(charWrapper ? "" : wordText));
          } else {
            curWordEl = wordWrapper(charWrapper ? "" : wordText);
            _insertNodeBefore(curWordEl, element, curNode);
            ignoredPreviousSibling && j === 1 && !startsWithSpace && curWordEl.insertBefore(ignoredPreviousSibling, curWordEl.firstChild);
          }
          if (charWrapper) {
            curWordChars = _charSegmenter ? _stretchToFitSpecialChars([..._charSegmenter.segment(wordText)].map((s) => s.segment), specialCharsRegEx) : wordText.match(charSplitRegEx) || _emptyArray2;
            for (k = 0; k < curWordChars.length; k++) {
              curWordEl.appendChild(curWordChars[k] === " " ? document.createTextNode(" ") : charWrapper(curWordChars[k]));
            }
          }
          if (deepSlice && isNested) {
            curTextContent = curNode.textContent = curTextContent.substring(wordText.length + 1, curTextContent.length);
            bounds = curWordEl.getBoundingClientRect();
            if (bounds.top > lastBounds.top && bounds.left <= lastBounds.left) {
              clonedNode = element.cloneNode();
              curSubNode = element.childNodes[0];
              while (curSubNode && curSubNode !== curWordEl) {
                tempSubNode = curSubNode;
                curSubNode = curSubNode.nextSibling;
                clonedNode.appendChild(tempSubNode);
              }
              element.parentNode.insertBefore(clonedNode, element);
              prepForCharsOnly && _disallowInline(clonedNode);
            }
            lastBounds = bounds;
          }
          if (j < words.length || endsWithSpace) {
            _insertNodeBefore(j >= words.length ? " " : wordDelimIsNotSpace && wordText.slice(-1) === " " ? " " + wordDelimString : wordDelimString, element, curNode);
          }
        }
      }
      element.removeChild(curNode);
      ignoredPreviousSibling = 0;
    } else if (curNode.nodeType === 1) {
      if (ignore && ignore.indexOf(curNode) > -1) {
        wordsCollection.indexOf(curNode.previousSibling) > -1 && wordsCollection[wordsCollection.length - 1].appendChild(curNode);
        ignoredPreviousSibling = curNode;
      } else {
        _splitWordsAndCharsRecursively(curNode, config3, wordWrapper, charWrapper, prepForCharsOnly, deepSlice, ignore, charSplitRegEx, specialCharsRegEx, true);
        ignoredPreviousSibling = 0;
      }
      prepForCharsOnly && _disallowInline(curNode);
    }
  }
};
var _SplitText = class _SplitText2 {
  constructor(elements, config3) {
    this.isSplit = false;
    _initIfNecessary();
    this.elements = _elements(elements);
    this.chars = [];
    this.words = [];
    this.lines = [];
    this.masks = [];
    this.vars = config3;
    this.elements.forEach((el) => {
      var _a;
      config3.overwrite !== false && ((_a = el[_splitProp]) == null ? void 0 : _a._data.orig.filter(({ element }) => element === el).forEach(_revertOriginal));
      el[_splitProp] = this;
    });
    this._split = () => this.isSplit && this.split(this.vars);
    let orig = [], timerId, checkWidths = () => {
      let i2 = orig.length, o;
      while (i2--) {
        o = orig[i2];
        let w = o.element.offsetWidth;
        if (w !== o.width) {
          o.width = w;
          this._split();
          return;
        }
      }
    };
    this._data = { orig, obs: typeof ResizeObserver !== "undefined" && new ResizeObserver(() => {
      clearTimeout(timerId);
      timerId = setTimeout(checkWidths, 200);
    }) };
    _context2(this);
    this.split(config3);
  }
  split(config3) {
    (this._ctx || _defaultContext).add(() => {
      this.isSplit && this.revert();
      this.vars = config3 = config3 || this.vars || {};
      let { type = "chars,words,lines", aria = "auto", deepSlice = true, smartWrap, onSplit, autoSplit = false, specialChars, mask } = this.vars, splitLines = type.indexOf("lines") > -1, splitCharacters = type.indexOf("chars") > -1, splitWords = type.indexOf("words") > -1, onlySplitCharacters = splitCharacters && !splitWords && !splitLines, specialCharsRegEx = specialChars && ("push" in specialChars ? new RegExp("(?:" + specialChars.join("|") + ")", "gu") : specialChars), finalCharSplitRegEx = specialCharsRegEx ? new RegExp(specialCharsRegEx.source + "|" + _emojiSafeRegEx.source, "gu") : _emojiSafeRegEx, ignore = !!config3.ignore && _elements(config3.ignore), { orig, animTime, obs } = this._data, onSplitResult;
      if (splitCharacters || splitWords || splitLines) {
        this.elements.forEach((element, index) => {
          orig[index] = {
            element,
            html: element.innerHTML,
            ariaL: element.getAttribute("aria-label"),
            ariaH: element.getAttribute("aria-hidden")
          };
          aria === "auto" ? element.setAttribute("aria-label", (element.textContent || "").trim()) : aria === "hidden" && element.setAttribute("aria-hidden", "true");
          let chars = [], words = [], lines = [], charWrapper = splitCharacters ? _getWrapper("char", config3, chars) : null, wordWrapper = _getWrapper("word", config3, words), i2, curWord, smartWrapSpan, nextSibling;
          _splitWordsAndCharsRecursively(element, config3, wordWrapper, charWrapper, onlySplitCharacters, deepSlice && (splitLines || onlySplitCharacters), ignore, finalCharSplitRegEx, specialCharsRegEx, false);
          if (splitLines) {
            let nodes = _toArray(element.childNodes), wrapLine = _getLineWrapper(element, nodes, config3, lines), curNode, toRemove = [], lineStartIndex = 0, allBounds = nodes.map((n) => n.nodeType === 1 ? n.getBoundingClientRect() : _emptyBounds), lastBounds = _emptyBounds, curBounds;
            for (i2 = 0; i2 < nodes.length; i2++) {
              curNode = nodes[i2];
              if (curNode.nodeType === 1) {
                if (curNode.nodeName === "BR") {
                  if (!i2 || nodes[i2 - 1].nodeName !== "BR") {
                    toRemove.push(curNode);
                    wrapLine(lineStartIndex, i2 + 1);
                  }
                  lineStartIndex = i2 + 1;
                  lastBounds = _findNextValidBounds(allBounds, i2);
                } else {
                  curBounds = allBounds[i2];
                  if (i2 && curBounds.top > lastBounds.top && curBounds.left < lastBounds.left + lastBounds.width - 1) {
                    wrapLine(lineStartIndex, i2);
                    lineStartIndex = i2;
                  }
                  lastBounds = curBounds;
                }
              }
            }
            lineStartIndex < i2 && wrapLine(lineStartIndex, i2);
            toRemove.forEach((el) => {
              var _a;
              return (_a = el.parentNode) == null ? void 0 : _a.removeChild(el);
            });
          }
          if (!splitWords) {
            for (i2 = 0; i2 < words.length; i2++) {
              curWord = words[i2];
              if (splitCharacters || !curWord.nextSibling || curWord.nextSibling.nodeType !== 3) {
                if (smartWrap && !splitLines) {
                  smartWrapSpan = document.createElement("span");
                  smartWrapSpan.style.whiteSpace = "nowrap";
                  while (curWord.firstChild) {
                    smartWrapSpan.appendChild(curWord.firstChild);
                  }
                  curWord.replaceWith(smartWrapSpan);
                } else {
                  curWord.replaceWith(...curWord.childNodes);
                }
              } else {
                nextSibling = curWord.nextSibling;
                if (nextSibling && nextSibling.nodeType === 3) {
                  nextSibling.textContent = (curWord.textContent || "") + (nextSibling.textContent || "");
                  curWord.remove();
                }
              }
            }
            words.length = 0;
            element.normalize();
          }
          this.lines.push(...lines);
          this.words.push(...words);
          this.chars.push(...chars);
        });
        mask && this[mask] && this.masks.push(...this[mask].map((el) => {
          let maskEl = el.cloneNode();
          el.replaceWith(maskEl);
          maskEl.appendChild(el);
          el.className && (maskEl.className = el.className.trim().split(" ").map((s) => s + "-mask").join(" "));
          maskEl.style.overflow = "clip";
          return maskEl;
        }));
      }
      this.isSplit = true;
      _fonts && splitLines && autoSplit && _fonts.addEventListener("loadingdone", this._split);
      if ((onSplitResult = onSplit && onSplit(this)) && onSplitResult.totalTime) {
        this._data.anim = animTime ? onSplitResult.totalTime(animTime) : onSplitResult;
      }
      splitLines && autoSplit && this.elements.forEach((element, index) => {
        orig[index].width = element.offsetWidth;
        obs && obs.observe(element);
      });
    });
    return this;
  }
  kill() {
    let { obs } = this._data;
    obs && obs.disconnect();
    _fonts == null ? void 0 : _fonts.removeEventListener("loadingdone", this._split);
  }
  revert() {
    var _a, _b;
    if (this.isSplit) {
      let { orig, anim } = this._data;
      this.kill();
      orig.forEach(_revertOriginal);
      this.chars.length = this.words.length = this.lines.length = orig.length = this.masks.length = 0;
      this.isSplit = false;
      if (anim) {
        this._data.animTime = anim.totalTime();
        anim.revert();
      }
      (_b = (_a = this.vars).onRevert) == null ? void 0 : _b.call(_a, this);
    }
    return this;
  }
  static create(elements, config3) {
    return new _SplitText2(elements, config3);
  }
  static register(core) {
    gsap6 = gsap6 || core || window.gsap;
    if (gsap6) {
      _toArray = gsap6.utils.toArray;
      _context2 = gsap6.core.context || _context2;
    }
    if (!_coreInitted6 && window.innerWidth > 0) {
      _fonts = document.fonts;
      _coreInitted6 = true;
    }
  }
};
_SplitText.version = "3.15.0";
var SplitText = _SplitText;

// examples/figma-launch/node_modules/gsap/MorphSVGPlugin.js
function _extends() {
  _extends = Object.assign || function(target) {
    for (var i2 = 1; i2 < arguments.length; i2++) {
      var source = arguments[i2];
      for (var key in source) {
        if (Object.prototype.hasOwnProperty.call(source, key)) {
          target[key] = source[key];
        }
      }
    }
    return target;
  };
  return _extends.apply(this, arguments);
}
var gsap7;
var _toArray2;
var _lastLinkedAnchor;
var _doc3;
var _coreInitted7;
var PluginClass;
var _reverting3;
var _getGSAP9 = function _getGSAP10() {
  return gsap7 || typeof window !== "undefined" && (gsap7 = window.gsap) && gsap7.registerPlugin && gsap7;
};
var _isFunction3 = function _isFunction4(value) {
  return typeof value === "function";
};
var _atan23 = Math.atan2;
var _cos3 = Math.cos;
var _sin3 = Math.sin;
var _sqrt3 = Math.sqrt;
var _PI = Math.PI;
var _2PI2 = _PI * 2;
var _angleMin = _PI * 0.3;
var _angleMax = _PI * 0.7;
var _bigNum4 = 1e20;
var _numExp3 = /[-+=.]*\d+[.e\-+]*\d*[e\-+]*\d*/gi;
var _selectorExp2 = /(^[#.][a-z]|[a-y][a-z])/i;
var _commands = /[achlmqstvz]/i;
var _log = function _log2(message) {
  return console && console.warn(message);
};
var _round7 = function _round8(value) {
  return Math.round(value * 1e5) / 1e5 || 0;
};
var _getAverageXY = function _getAverageXY2(segment) {
  var l = segment.length, x = 0, y = 0, i2;
  for (i2 = 0; i2 < l; i2++) {
    x += segment[i2++];
    y += segment[i2];
  }
  return [x / (l / 2), y / (l / 2)];
};
var _getSize = function _getSize2(segment) {
  var l = segment.length, xMax = segment[0], xMin = xMax, yMax = segment[1], yMin = yMax, x, y, i2;
  for (i2 = 6; i2 < l; i2 += 6) {
    x = segment[i2];
    y = segment[i2 + 1];
    if (x > xMax) {
      xMax = x;
    } else if (x < xMin) {
      xMin = x;
    }
    if (y > yMax) {
      yMax = y;
    } else if (y < yMin) {
      yMin = y;
    }
  }
  segment.centerX = (xMax + xMin) / 2;
  segment.centerY = (yMax + yMin) / 2;
  return segment.size = (xMax - xMin) * (yMax - yMin);
};
var _getTotalSize = function _getTotalSize2(rawPath, samplesPerBezier) {
  if (samplesPerBezier === void 0) {
    samplesPerBezier = 3;
  }
  var j = rawPath.length, xMax = rawPath[0][0], xMin = xMax, yMax = rawPath[0][1], yMin = yMax, inc = 1 / samplesPerBezier, l, x, y, i2, segment, k, t, inv, x1, y1, x2, x3, x4, y2, y3, y4;
  while (--j > -1) {
    segment = rawPath[j];
    l = segment.length;
    for (i2 = 6; i2 < l; i2 += 6) {
      x1 = segment[i2];
      y1 = segment[i2 + 1];
      x2 = segment[i2 + 2] - x1;
      y2 = segment[i2 + 3] - y1;
      x3 = segment[i2 + 4] - x1;
      y3 = segment[i2 + 5] - y1;
      x4 = segment[i2 + 6] - x1;
      y4 = segment[i2 + 7] - y1;
      k = samplesPerBezier;
      while (--k > -1) {
        t = inc * k;
        inv = 1 - t;
        x = (t * t * x4 + 3 * inv * (t * x3 + inv * x2)) * t + x1;
        y = (t * t * y4 + 3 * inv * (t * y3 + inv * y2)) * t + y1;
        if (x > xMax) {
          xMax = x;
        } else if (x < xMin) {
          xMin = x;
        }
        if (y > yMax) {
          yMax = y;
        } else if (y < yMin) {
          yMin = y;
        }
      }
    }
  }
  rawPath.centerX = (xMax + xMin) / 2;
  rawPath.centerY = (yMax + yMin) / 2;
  rawPath.left = xMin;
  rawPath.width = xMax - xMin;
  rawPath.top = yMin;
  rawPath.height = yMax - yMin;
  return rawPath.size = (xMax - xMin) * (yMax - yMin);
};
var _sortByComplexity = function _sortByComplexity2(a, b) {
  return b.length - a.length;
};
var _sortBySize = function _sortBySize2(a, b) {
  var sizeA = a.size || _getSize(a), sizeB = b.size || _getSize(b);
  return Math.abs(sizeB - sizeA) < (sizeA + sizeB) / 20 ? b.centerX - a.centerX || b.centerY - a.centerY : sizeB - sizeA;
};
var _offsetSegment = function _offsetSegment2(segment, shapeIndex) {
  var a = segment.slice(0), l = segment.length, wrap3 = l - 2, i2, index;
  shapeIndex = shapeIndex | 0;
  for (i2 = 0; i2 < l; i2++) {
    index = (i2 + shapeIndex) % wrap3;
    segment[i2++] = a[index];
    segment[i2] = a[index + 1];
  }
};
var _getTotalMovement = function _getTotalMovement2(sb, eb, shapeIndex, offsetX, offsetY) {
  var l = sb.length, d = 0, wrap3 = l - 2, index, i2, x, y;
  shapeIndex *= 6;
  for (i2 = 0; i2 < l; i2 += 6) {
    index = (i2 + shapeIndex) % wrap3;
    y = sb[index] - (eb[i2] - offsetX);
    x = sb[index + 1] - (eb[i2 + 1] - offsetY);
    d += _sqrt3(x * x + y * y);
  }
  return d;
};
var _getClosestShapeIndex = function _getClosestShapeIndex2(sb, eb, checkReverse) {
  var l = sb.length, sCenter = _getAverageXY(sb), eCenter = _getAverageXY(eb), offsetX = eCenter[0] - sCenter[0], offsetY = eCenter[1] - sCenter[1], min = _getTotalMovement(sb, eb, 0, offsetX, offsetY), minIndex = 0, copy, d, i2;
  for (i2 = 6; i2 < l; i2 += 6) {
    d = _getTotalMovement(sb, eb, i2 / 6, offsetX, offsetY);
    if (d < min) {
      min = d;
      minIndex = i2;
    }
  }
  if (checkReverse) {
    copy = sb.slice(0);
    reverseSegment(copy);
    for (i2 = 6; i2 < l; i2 += 6) {
      d = _getTotalMovement(copy, eb, i2 / 6, offsetX, offsetY);
      if (d < min) {
        min = d;
        minIndex = -i2;
      }
    }
  }
  return minIndex / 6;
};
var _getClosestAnchor = function _getClosestAnchor2(rawPath, x, y) {
  var j = rawPath.length, closestDistance = _bigNum4, closestX = 0, closestY = 0, segment, dx, dy, d, i2, l;
  while (--j > -1) {
    segment = rawPath[j];
    l = segment.length;
    for (i2 = 0; i2 < l; i2 += 6) {
      dx = segment[i2] - x;
      dy = segment[i2 + 1] - y;
      d = _sqrt3(dx * dx + dy * dy);
      if (d < closestDistance) {
        closestDistance = d;
        closestX = segment[i2];
        closestY = segment[i2 + 1];
      }
    }
  }
  return [closestX, closestY];
};
var _getClosestSegment = function _getClosestSegment2(bezier, pool, startIndex, sortRatio, offsetX, offsetY) {
  var l = pool.length, index = 0, minSize = Math.min(bezier.size || _getSize(bezier), pool[startIndex].size || _getSize(pool[startIndex])) * sortRatio, min = _bigNum4, cx = bezier.centerX + offsetX, cy = bezier.centerY + offsetY, size, i2, dx, dy, d;
  for (i2 = startIndex; i2 < l; i2++) {
    size = pool[i2].size || _getSize(pool[i2]);
    if (size < minSize) {
      break;
    }
    dx = pool[i2].centerX - cx;
    dy = pool[i2].centerY - cy;
    d = _sqrt3(dx * dx + dy * dy);
    if (d < min) {
      index = i2;
      min = d;
    }
  }
  d = pool[index];
  pool.splice(index, 1);
  return d;
};
var _addAnchorsToBezier = function _addAnchorsToBezier2(segment, i2, quantity) {
  if (quantity === void 0) {
    quantity = 1;
  }
  var ax = segment[i2], ay = segment[i2 + 1], cp1x = segment[i2 + 2], cp1y = segment[i2 + 3], cp2x = segment[i2 + 4], cp2y = segment[i2 + 5], bx = segment[i2 + 6], by = segment[i2 + 7], t, x1a, x2, y1a, y2, x1, y1, x2a, y2a;
  while (quantity-- > 0) {
    t = 1 - 1 / (quantity + 2);
    x1a = ax + (cp1x - ax) * t;
    x2 = cp1x + (cp2x - cp1x) * t;
    y1a = ay + (cp1y - ay) * t;
    y2 = cp1y + (cp2y - cp1y) * t;
    x1 = x1a + (x2 - x1a) * t;
    y1 = y1a + (y2 - y1a) * t;
    x2a = cp2x + (bx - cp2x) * t;
    y2a = cp2y + (by - cp2y) * t;
    x2 += (x2a - x2) * t;
    y2 += (y2a - y2) * t;
    segment.splice(
      i2 + 2,
      4,
      cp1x = _round7(x1a),
      // first control point
      cp1y = _round7(y1a),
      cp2x = _round7(x1),
      // second control point
      cp2y = _round7(y1),
      bx = _round7(x1 + (x2 - x1) * t),
      // new fabricated anchor
      by = _round7(y1 + (y2 - y1) * t),
      _round7(x2),
      // third control point
      _round7(y2),
      _round7(x2a),
      // fourth control point
      _round7(y2a)
    );
  }
};
var _getLargestIndex = function _getLargestIndex2(a) {
  var i2 = a.length, max = -_bigNum4, largestIndex;
  while (i2--) {
    if (a[i2] > max) {
      max = a[i2];
      largestIndex = i2;
    }
  }
  return largestIndex;
};
var _subdivideSegmentQty = function _subdivideSegmentQty2(segment, quantity) {
  var distances = [], anchorsToAdd = [], l = segment.length - 2, i2 = 0;
  for (; i2 < l; i2 += 6) {
    distances.push(Math.pow(segment[i2] - segment[i2 + 6], 2) + Math.pow(segment[i2 + 1] - segment[i2 + 7], 2));
  }
  while (quantity--) {
    i2 = _getLargestIndex(distances);
    anchorsToAdd[i2] = l = (anchorsToAdd[i2] || 0) + 1;
    distances[i2] *= l / (l + 1);
  }
  i2 = distances.length;
  while (i2--) {
    anchorsToAdd[i2] && _addAnchorsToBezier(segment, i2 * 6, anchorsToAdd[i2]);
  }
};
var _getDefaultSmoothPoints = function _getDefaultSmoothPoints2(rawPath, skipMeasure) {
  skipMeasure || cacheRawPathMeasurements(rawPath);
  return Math.max(4, Math.round(rawPath.totalLength / 4));
};
var _cloneAndSortRawPath = function _cloneAndSortRawPath2(ar) {
  return ar.slice(0).sort(_sortByComplexity);
};
var _segmentCanBeIgnored = function _segmentCanBeIgnored2(segment) {
  var x = segment[0], y = segment[1], i2 = 2;
  for (; i2 < segment.length; i2 += 2) {
    if (Math.abs(segment[i2] - x) > 0.01 || Math.abs(segment[i2 + 1] - y) > 0.01) {
      return false;
    }
  }
  return true;
};
var _smoothRawPath = function _smoothRawPath2(rawPath, config3) {
  config3 = config3 || {};
  var _config4 = config3, redraw = _config4.redraw, points = _config4.points, _config$maxSegments = _config4.maxSegments, maxSegments = _config$maxSegments === void 0 ? 999 : _config$maxSegments, pointsAdded = 0, sortedRawPath = rawPath, templateRawPath = Array.isArray(points) ? points : 0, segmentPointsToAdd, j, segment, smoothSegment, anchorDistance;
  redraw = redraw !== false;
  if (redraw) {
    cacheRawPathMeasurements(rawPath);
  } else {
    rawPath.totalPoints = 0;
    j = rawPath.length;
    while (j--) {
      rawPath.totalPoints += rawPath[j].length;
    }
  }
  if (templateRawPath) {
    sortedRawPath = _cloneAndSortRawPath(rawPath);
    templateRawPath = _cloneAndSortRawPath(templateRawPath);
    anchorDistance = templateRawPath[0].totalLength / Math.round(templateRawPath[0].length / 6);
  } else {
    if (!points || points === "auto") {
      points = _getDefaultSmoothPoints(rawPath, redraw);
      redraw || (points -= Math.round(rawPath.totalPoints / 6));
    }
    points = Math.max(redraw ? 10 : 4, Math.min(999, points));
  }
  for (j = 0; j < sortedRawPath.length; j++) {
    segment = sortedRawPath[j];
    segmentPointsToAdd = Math.max(redraw ? 10 : 4, templateRawPath ? Math.round(templateRawPath[j] ? templateRawPath[j].length / 6 : sortedRawPath[j].totalLength / anchorDistance || 0) : Math.round((pointsAdded / points + (redraw ? segment.totalLength / rawPath.totalLength : segment.length / rawPath.totalPoints)) * points) - pointsAdded);
    if (j >= maxSegments || templateRawPath && (!templateRawPath[j] || _segmentCanBeIgnored(templateRawPath[j]))) {
    } else if (redraw) {
      var _segment;
      smoothSegment = pointsToSegment(segmentToDistributedPoints(segment, segmentPointsToAdd), config3.curviness);
      segment.length = 0;
      (_segment = segment).push.apply(_segment, smoothSegment);
    } else {
      _subdivideSegmentQty(segment, segmentPointsToAdd);
    }
    pointsAdded += segmentPointsToAdd;
  }
  return rawPath;
};
var _equalizeSegmentQuantity = function _equalizeSegmentQuantity2(start, end, shapeIndex, map, fillSafe) {
  var dif = end.length - start.length, longer = dif > 0 ? end : start, shorter = dif > 0 ? start : end, added = 0, sortMethod = map === "complexity" ? _sortByComplexity : _sortBySize, sortRatio = map === "position" ? 0 : typeof map === "number" ? map : 0.8, i2 = shorter.length, shapeIndices = typeof shapeIndex === "object" && shapeIndex.push ? shapeIndex.slice(0) : [shapeIndex], reverse = shapeIndices[0] === "reverse" || shapeIndices[0] < 0, log = shapeIndex === "log", eb, sb, b, x, y, offsetX, offsetY;
  if (!shorter[0]) {
    return;
  }
  if (longer.length > 1) {
    start.sort(sortMethod);
    end.sort(sortMethod);
    longer.size || _getTotalSize(longer);
    shorter.size || _getTotalSize(shorter);
    offsetX = longer.centerX - shorter.centerX;
    offsetY = longer.centerY - shorter.centerY;
    if (sortMethod === _sortBySize) {
      for (i2 = 0; i2 < shorter.length; i2++) {
        longer.splice(i2, 0, _getClosestSegment(shorter[i2], longer, i2, sortRatio, offsetX, offsetY));
      }
    }
  }
  if (dif) {
    dif < 0 && (dif = -dif);
    longer[0].length > shorter[0].length && _subdivideSegmentQty(shorter[0], (longer[0].length - shorter[0].length) / 6 | 0);
    i2 = shorter.length;
    while (added < dif) {
      x = longer[i2].size || _getSize(longer[i2]);
      b = _getClosestAnchor(shorter, longer[i2].centerX, longer[i2].centerY);
      x = b[0];
      y = b[1];
      shorter[i2++] = [x, y, x, y, x, y, x, y];
      shorter.totalPoints += 8;
      added++;
    }
  }
  for (i2 = 0; i2 < start.length; i2++) {
    eb = end[i2];
    sb = start[i2];
    dif = eb.length - sb.length;
    if (dif < 0) {
      _subdivideSegmentQty(eb, -dif / 6 | 0);
    } else if (dif > 0) {
      _subdivideSegmentQty(sb, dif / 6 | 0);
    }
    if (reverse && fillSafe !== false && !sb.reversed) {
      reverseSegment(sb);
    }
    shapeIndex = shapeIndices[i2] || shapeIndices[i2] === 0 ? shapeIndices[i2] : "auto";
    if (shapeIndex) {
      if (sb.closed || Math.abs(sb[0] - sb[sb.length - 2]) < 0.5 && Math.abs(sb[1] - sb[sb.length - 1]) < 0.5) {
        if (shapeIndex === "auto" || shapeIndex === "log") {
          shapeIndices[i2] = shapeIndex = _getClosestShapeIndex(sb, eb, !i2 || fillSafe === false);
          if (shapeIndex < 0) {
            reverse = true;
            reverseSegment(sb);
            shapeIndex = -shapeIndex;
          }
          _offsetSegment(sb, shapeIndex * 6);
        } else if (shapeIndex !== "reverse") {
          if (i2 && shapeIndex < 0) {
            reverseSegment(sb);
          }
          _offsetSegment(sb, (shapeIndex < 0 ? -shapeIndex : shapeIndex) * 6);
        }
      } else if (!reverse && (shapeIndex === "auto" && Math.abs(eb[0] - sb[0]) + Math.abs(eb[1] - sb[1]) + Math.abs(eb[eb.length - 2] - sb[sb.length - 2]) + Math.abs(eb[eb.length - 1] - sb[sb.length - 1]) > Math.abs(eb[0] - sb[sb.length - 2]) + Math.abs(eb[1] - sb[sb.length - 1]) + Math.abs(eb[eb.length - 2] - sb[0]) + Math.abs(eb[eb.length - 1] - sb[1]) || shapeIndex % 2)) {
        reverseSegment(sb);
        shapeIndices[i2] = -1;
        reverse = true;
      } else if (shapeIndex === "auto") {
        shapeIndices[i2] = 0;
      } else if (shapeIndex === "reverse") {
        shapeIndices[i2] = -1;
      }
      if (sb.closed !== eb.closed) {
        sb.closed = eb.closed = false;
      }
    }
  }
  log && _log("shapeIndex:[" + shapeIndices.join(",") + "]");
  start.shapeIndex = shapeIndices;
  return shapeIndices;
};
var _pathFilter = function _pathFilter2(a, shapeIndex, map, precompile, fillSafe) {
  var start = stringToRawPath(a[0]), end = stringToRawPath(a[1]);
  if (!_equalizeSegmentQuantity(start, end, shapeIndex || shapeIndex === 0 ? shapeIndex : "auto", map, fillSafe)) {
    return;
  }
  a[0] = rawPathToString(start);
  a[1] = rawPathToString(end);
  (precompile === "log" || precompile === true) && _log('precompile:["' + a[0] + '","' + a[1] + '"]');
};
var _offsetPoints = function _offsetPoints2(text, offset) {
  if (!offset) {
    return text;
  }
  var a = text.match(_numExp3) || [], l = a.length, s = "", inc, i2, j;
  if (offset === "reverse") {
    i2 = l - 1;
    inc = -2;
  } else {
    i2 = ((parseInt(offset, 10) || 0) * 2 + 1 + l * 100) % l;
    inc = 2;
  }
  for (j = 0; j < l; j += 2) {
    s += a[i2 - 1] + "," + a[i2] + " ";
    i2 = (i2 + inc) % l;
  }
  return s;
};
var _equalizePointQuantity = function _equalizePointQuantity2(a, quantity) {
  var tally = 0, x = parseFloat(a[0]), y = parseFloat(a[1]), s = x + "," + y + " ", max = 0.999999, newPointsPerSegment, i2, l, j, factor, nextX, nextY;
  l = a.length;
  newPointsPerSegment = quantity * 0.5 / (l * 0.5 - 1);
  for (i2 = 0; i2 < l - 2; i2 += 2) {
    tally += newPointsPerSegment;
    nextX = parseFloat(a[i2 + 2]);
    nextY = parseFloat(a[i2 + 3]);
    if (tally > max) {
      factor = 1 / (Math.floor(tally) + 1);
      j = 1;
      while (tally > max) {
        s += (x + (nextX - x) * factor * j).toFixed(2) + "," + (y + (nextY - y) * factor * j).toFixed(2) + " ";
        tally--;
        j++;
      }
    }
    s += nextX + "," + nextY + " ";
    x = nextX;
    y = nextY;
  }
  return s;
};
var _pointsFilter = function _pointsFilter2(a) {
  var startNums = a[0].match(_numExp3) || [], endNums = a[1].match(_numExp3) || [], dif = endNums.length - startNums.length;
  if (dif > 0) {
    a[0] = _equalizePointQuantity(startNums, dif);
  } else {
    a[1] = _equalizePointQuantity(endNums, -dif);
  }
};
var _buildPointsFilter = function _buildPointsFilter2(shapeIndex) {
  return !isNaN(shapeIndex) ? function(a) {
    _pointsFilter(a);
    a[1] = _offsetPoints(a[1], parseInt(shapeIndex, 10));
  } : _pointsFilter;
};
var _parseShape = function _parseShape2(shape, forcePath, target) {
  var isString = typeof shape === "string", e, type;
  if (!isString || _selectorExp2.test(shape) || (shape.match(_numExp3) || []).length < 3) {
    e = _toArray2(shape)[0];
    if (e) {
      type = (e.nodeName + "").toUpperCase();
      if (forcePath && type !== "PATH") {
        e = convertToPath(e, false);
        type = "PATH";
      }
      shape = e.getAttribute(type === "PATH" ? "d" : "points") || "";
      if (e === target) {
        shape = e.getAttributeNS(null, "data-original") || shape;
      }
    } else {
      _log("WARNING: invalid morph to: " + shape);
      shape = false;
    }
  }
  return shape;
};
var _recordControlPointData = function _recordControlPointData2(rawPath) {
  var j = rawPath.length, segment, x, y, x2, y2, i2, l, cpData;
  while (--j > -1) {
    segment = rawPath[j];
    cpData = segment.cpData = segment.cpData || [];
    cpData.length = 0;
    l = segment.length - 2;
    for (i2 = 0; i2 < l; i2 += 6) {
      x = segment[i2] - segment[i2 + 2];
      y = segment[i2 + 1] - segment[i2 + 3];
      x2 = segment[i2 + 6] - segment[i2 + 4];
      y2 = segment[i2 + 7] - segment[i2 + 5];
      cpData[i2 + 2] = _atan23(y, x);
      cpData[i2 + 3] = _sqrt3(x * x + y * y);
      cpData[i2 + 4] = _atan23(y2, x2);
      cpData[i2 + 5] = _sqrt3(x2 * x2 + y2 * y2);
    }
  }
  return rawPath;
};
var _parseOriginFactors = function _parseOriginFactors2(v) {
  var a = v.trim().split(" "), x = ~v.indexOf("left") ? 0 : ~v.indexOf("right") ? 100 : isNaN(parseFloat(a[0])) ? 50 : parseFloat(a[0]), y = ~v.indexOf("top") ? 0 : ~v.indexOf("bottom") ? 100 : isNaN(parseFloat(a[1])) ? 50 : parseFloat(a[1]);
  return {
    x: x / 100,
    y: y / 100
  };
};
var _shortAngle = function _shortAngle2(dif) {
  return dif !== dif % _PI ? dif + (dif < 0 ? _2PI2 : -_2PI2) : dif;
};
var _morphMessage = "Use MorphSVGPlugin.convertToPath() to convert to a path before morphing.";
var _tweenRotation = function _tweenRotation2(start, end, i2, linkedPT) {
  var so = this._origin, eo = this._eOrigin, dx = start[i2] - so.x, dy = start[i2 + 1] - so.y, d = _sqrt3(dx * dx + dy * dy), sa = _atan23(dy, dx), angleDif, _short;
  dx = end[i2] - eo.x;
  dy = end[i2 + 1] - eo.y;
  angleDif = _atan23(dy, dx) - sa;
  _short = _shortAngle(angleDif);
  if (!linkedPT && _lastLinkedAnchor && Math.abs(_short + _lastLinkedAnchor.ca) < _angleMin) {
    linkedPT = _lastLinkedAnchor;
  }
  return this._anchorPT = _lastLinkedAnchor = {
    _next: this._anchorPT,
    t: start,
    sa,
    // starting angle
    ca: linkedPT && _short * linkedPT.ca < 0 && Math.abs(_short) > _angleMax ? angleDif : _short,
    //change in angle
    sl: d,
    // starting length
    cl: _sqrt3(dx * dx + dy * dy) - d,
    // change in length
    i: i2
  };
};
var _initCore11 = function _initCore12(required) {
  gsap7 = _getGSAP9();
  PluginClass = PluginClass || gsap7 && gsap7.plugins.morphSVG;
  if (gsap7 && PluginClass) {
    _toArray2 = gsap7.utils.toArray;
    _reverting3 = gsap7.core.reverting || function() {
    };
    _doc3 = document;
    PluginClass.prototype._tweenRotation = _tweenRotation;
    _coreInitted7 = 1;
  } else if (required) {
    _log("Please gsap.registerPlugin(MorphSVGPlugin)");
  }
};
var MorphSVGPlugin = {
  version: "3.15.0",
  name: "morphSVG",
  rawVars: 1,
  // otherwise "render" would be interpreted as a function-based value.
  register: function register(core, Plugin) {
    gsap7 = core;
    PluginClass = Plugin;
    _initCore11();
  },
  init: function init4(target, value, tween, index, targets) {
    _coreInitted7 || _initCore11(1);
    if (!value) {
      _log("invalid shape");
      return false;
    }
    _isFunction3(value) && (value = value.call(tween, index, target, targets));
    var type, p2, pt, shape, isPoly, shapeIndex, map, startCPData, endCPData, start, end, i2, j, l, startSeg, endSeg, precompiled, originFactors, useRotation, curveMode;
    if (typeof value === "string" || value.getBBox || value[0]) {
      value = {
        shape: value
      };
    } else if (typeof value === "object") {
      type = {};
      for (p2 in value) {
        type[p2] = _isFunction3(value[p2]) && p2 !== "render" ? value[p2].call(tween, index, target, targets) : value[p2];
      }
      value = type;
    }
    var cs = target.nodeType ? window.getComputedStyle(target) : {}, fill = cs.fill + "", fillSafe = !(fill === "none" || (fill.match(_numExp3) || [])[3] === "0" || cs.fillRule === "evenodd"), smooth = value.smooth, origins = (value.origin || "50 50").split(",");
    smooth === true || smooth === "auto" ? smooth = {} : typeof smooth === "number" && (smooth = {
      points: smooth
    });
    type = (target.nodeName + "").toUpperCase();
    isPoly = type === "POLYLINE" || type === "POLYGON";
    if (type !== "PATH" && !isPoly && !value.prop) {
      _log("Cannot morph a <" + type + "> element. " + _morphMessage);
      return false;
    }
    p2 = type === "PATH" ? "d" : "points";
    if (!value.prop && !_isFunction3(target.setAttribute)) {
      return false;
    }
    shape = _parseShape(value.shape || value.d || value.points || "", p2 === "d", target);
    if (isPoly && _commands.test(shape)) {
      _log("A <" + type + "> cannot accept path data. " + _morphMessage);
      return false;
    }
    shapeIndex = value.shapeIndex || value.shapeIndex === 0 ? value.shapeIndex : "auto";
    map = value.map || MorphSVGPlugin.defaultMap;
    this._prop = value.prop;
    this._render = value.render || MorphSVGPlugin.defaultRender;
    this._apply = "updateTarget" in value ? value.updateTarget : MorphSVGPlugin.defaultUpdateTarget;
    this._rnd = Math.pow(10, isNaN(value.precision) ? 2 : +value.precision);
    this._tween = tween;
    if (shape) {
      this._target = target;
      precompiled = typeof value.precompile === "object";
      start = this._original = this._prop ? target[this._prop] : target.getAttribute(p2);
      if (!this._prop && !target.getAttributeNS(null, "data-original")) {
        target.setAttributeNS(null, "data-original", start);
      }
      if (p2 === "d" || this._prop) {
        start = stringToRawPath(precompiled ? value.precompile[0] : start);
        end = stringToRawPath(precompiled ? value.precompile[1] : shape);
        if (smooth) {
          j = start.length;
          while (--j) {
            _segmentCanBeIgnored(start[j]) && start.splice(j, 1);
          }
          _smoothRawPath(start, _extends({}, smooth, {
            points: +smooth.points || Math.max(_getDefaultSmoothPoints(start), _getDefaultSmoothPoints(end)),
            maxSegments: end.length
          }));
          _smoothRawPath(end, smooth.redraw === false ? smooth : _extends({}, smooth, {
            points: start
          }));
        }
        if (!precompiled && !_equalizeSegmentQuantity(start, end, shapeIndex, map, fillSafe)) {
          return false;
        }
        if (value.precompile === "log" || value.precompile === true) {
          _log('precompile:["' + rawPathToString(start) + '","' + rawPathToString(end) + '"]');
        }
        useRotation = (value.type || MorphSVGPlugin.defaultType) !== "linear";
        curveMode = value.curveMode || useRotation;
        _recordControlPointData(start);
        _recordControlPointData(end);
        if (useRotation) {
          start.size || _getTotalSize(start);
          end.size || _getTotalSize(end);
          originFactors = _parseOriginFactors(origins[0]);
          this._origin = start.origin = {
            x: start.left + originFactors.x * start.width,
            y: start.top + originFactors.y * start.height
          };
          origins[1] && (originFactors = _parseOriginFactors(origins[1]));
          this._eOrigin = {
            x: end.left + originFactors.x * end.width,
            y: end.top + originFactors.y * end.height
          };
        }
        this._rawPath = target._gsRawPath = start;
        j = start.length;
        while (--j > -1) {
          startSeg = start[j];
          endSeg = end[j];
          startCPData = startSeg.cpData;
          endCPData = endSeg.cpData;
          l = startSeg.length;
          _lastLinkedAnchor = 0;
          for (i2 = 0; i2 < l; i2 += 6) {
            if (endSeg[i2] !== startSeg[i2] || endSeg[i2 + 1] !== startSeg[i2 + 1]) {
              if (useRotation) {
                pt = this._tweenRotation(startSeg, endSeg, i2);
              } else {
                pt = this.add(startSeg, i2, startSeg[i2], endSeg[i2], 0, 0, 0, 0, 0, 1);
                pt = this.add(startSeg, i2 + 1, startSeg[i2 + 1], endSeg[i2 + 1], 0, 0, 0, 0, 0, 1) || pt;
              }
            }
          }
          for (i2 = 0; i2 < l; i2 += 2) {
            if (curveMode && (startCPData[i2] !== endCPData[i2] || startCPData[i2 + 1] !== endCPData[i2 + 1]) && startCPData[i2 + 1] && endCPData[i2 + 1]) {
              this._controlPT = {
                _next: this._controlPT,
                i: i2,
                j,
                ai: i2 % 6 > 3 ? i2 + 2 : i2 - 2,
                sa: startCPData[i2],
                ca: _shortAngle(endCPData[i2] - startCPData[i2]),
                sl: startCPData[i2 + 1],
                cl: endCPData[i2 + 1] - startCPData[i2 + 1]
              };
            } else {
              endSeg[i2] !== startSeg[i2] && (pt = this.add(startSeg, i2, startSeg[i2], endSeg[i2], 0, 0, 0, 0, 0, 1));
              endSeg[i2 + 1] !== startSeg[i2 + 1] && (pt = this.add(startSeg, i2 + 1, startSeg[i2 + 1], endSeg[i2 + 1], 0, 0, 0, 0, 0, 1) || pt);
            }
          }
        }
      } else {
        pt = this.add(target, "setAttribute", target.getAttribute(p2) + "", shape + "", index, targets, 0, _buildPointsFilter(shapeIndex), p2);
      }
      if (useRotation) {
        this.add(this._origin, "x", this._origin.x, this._eOrigin.x, 0, 0, 0, 0, 0, 1);
        pt = this.add(this._origin, "y", this._origin.y, this._eOrigin.y, 0, 0, 0, 0, 0, 1);
      }
      if (pt) {
        this._props.push("morphSVG");
        pt.end = smooth && smooth.persist !== false ? rawPathToString(end) : shape;
        pt.endProp = p2;
      }
    }
    return 1;
  },
  render: function render3(ratio, data) {
    var rawPath = data._rawPath, controlPT = data._controlPT, anchorPT = data._anchorPT, rnd = data._rnd, target = data._target, pt = data._pt, s, space, segment, l, angle, i2, j, sin, cos;
    while (pt) {
      pt.r(ratio, pt.d);
      pt = pt._next;
    }
    if (ratio === 1 && data._apply) {
      pt = data._pt;
      while (pt) {
        if (pt.end) {
          if (data._prop) {
            target[data._prop] = pt.end;
          } else {
            target.setAttribute(pt.endProp, pt.end);
          }
        }
        pt = pt._next;
      }
    } else if (rawPath) {
      while (anchorPT) {
        angle = anchorPT.sa + ratio * anchorPT.ca;
        l = anchorPT.sl + ratio * anchorPT.cl;
        anchorPT.t[anchorPT.i] = data._origin.x + _cos3(angle) * l;
        anchorPT.t[anchorPT.i + 1] = data._origin.y + _sin3(angle) * l;
        anchorPT = anchorPT._next;
      }
      while (controlPT) {
        segment = rawPath[controlPT.j];
        i2 = controlPT.i;
        angle = controlPT.sa + ratio * controlPT.ca;
        sin = _sin3(angle);
        cos = _cos3(angle);
        l = controlPT.sl + ratio * controlPT.cl;
        segment[i2] = segment[controlPT.ai] - cos * l;
        segment[i2 + 1] = segment[controlPT.ai + 1] - sin * l;
        controlPT = controlPT._next;
      }
      if (!ratio && _reverting3()) {
        rawPath = stringToRawPath(data._original);
      }
      target._gsRawPath = rawPath;
      if (data._apply) {
        s = "";
        space = " ";
        for (j = 0; j < rawPath.length; j++) {
          segment = rawPath[j];
          l = segment.length;
          s += "M" + (segment[0] * rnd | 0) / rnd + space + (segment[1] * rnd | 0) / rnd + " C";
          for (i2 = 2; i2 < l; i2++) {
            s += (segment[i2] * rnd | 0) / rnd + space;
          }
          segment.closed && (s += "z");
        }
        if (data._prop) {
          target[data._prop] = s;
        } else {
          target.setAttribute("d", s);
        }
      }
    }
    data._render && rawPath && data._render.call(data._tween, rawPath, target);
  },
  kill: function kill(property) {
    this._pt = this._rawPath = 0;
  },
  getRawPath,
  stringToRawPath,
  rawPathToString,
  smoothRawPath: _smoothRawPath,
  normalizeStrings: function normalizeStrings(shape1, shape2, _ref) {
    var shapeIndex = _ref.shapeIndex, map = _ref.map;
    var result = [shape1, shape2];
    _pathFilter(result, shapeIndex, map);
    return result;
  },
  pathFilter: _pathFilter,
  pointsFilter: _pointsFilter,
  getTotalSize: _getTotalSize,
  equalizeSegmentQuantity: _equalizeSegmentQuantity,
  convertToPath: function convertToPath2(targets, swap) {
    return _toArray2(targets).map(function(target) {
      return convertToPath(target, swap !== false);
    });
  },
  defaultType: "linear",
  defaultUpdateTarget: true,
  defaultMap: "size"
};
_getGSAP9() && gsap7.registerPlugin(MorphSVGPlugin);

// examples/figma-launch/node_modules/gsap/DrawSVGPlugin.js
var gsap8;
var _toArray3;
var _doc4;
var _win3;
var _isEdge;
var _coreInitted8;
var _warned;
var _getStyleSaver3;
var _reverting4;
var _windowExists5 = function _windowExists6() {
  return typeof window !== "undefined";
};
var _getGSAP11 = function _getGSAP12() {
  return gsap8 || _windowExists5() && (gsap8 = window.gsap) && gsap8.registerPlugin && gsap8;
};
var _numExp4 = /[-+=\.]*\d+[\.e\-\+]*\d*[e\-\+]*\d*/gi;
var _types = {
  rect: ["width", "height"],
  circle: ["r", "r"],
  ellipse: ["rx", "ry"],
  line: ["x2", "y2"]
};
var _round9 = function _round10(value) {
  return Math.round(value * 1e4) / 1e4;
};
var _parseNum = function _parseNum2(value) {
  return parseFloat(value) || 0;
};
var _parseSingleVal = function _parseSingleVal2(value, length) {
  var num = _parseNum(value);
  return ~value.indexOf("%") ? num / 100 * length : num;
};
var _getAttributeAsNumber = function _getAttributeAsNumber2(target, attr) {
  return _parseNum(target.getAttribute(attr));
};
var _sqrt4 = Math.sqrt;
var _getDistance = function _getDistance2(x1, y1, x2, y2, scaleX, scaleY) {
  return _sqrt4(Math.pow((_parseNum(x2) - _parseNum(x1)) * scaleX, 2) + Math.pow((_parseNum(y2) - _parseNum(y1)) * scaleY, 2));
};
var _warn3 = function _warn4(message) {
  return console.warn(message);
};
var _hasNonScalingStroke = function _hasNonScalingStroke2(target) {
  return target.getAttribute("vector-effect") === "non-scaling-stroke";
};
var _bonusValidated4 = 1;
var _parse = function _parse2(value, length, defaultStart) {
  var i2 = value.indexOf(" "), s, e;
  if (i2 < 0) {
    s = defaultStart !== void 0 ? defaultStart + "" : value;
    e = value;
  } else {
    s = value.substr(0, i2);
    e = value.substr(i2 + 1);
  }
  s = _parseSingleVal(s, length);
  e = _parseSingleVal(e, length);
  return s > e ? [e, s] : [s, e];
};
var _getLength = function _getLength2(target) {
  target = _toArray3(target)[0];
  if (!target) {
    return 0;
  }
  var type = target.tagName.toLowerCase(), style = target.style, scaleX = 1, scaleY = 1, length, bbox, points, prevPoint, i2, rx, ry;
  if (_hasNonScalingStroke(target)) {
    scaleY = target.getScreenCTM();
    scaleX = _sqrt4(scaleY.a * scaleY.a + scaleY.b * scaleY.b);
    scaleY = _sqrt4(scaleY.d * scaleY.d + scaleY.c * scaleY.c);
  }
  try {
    bbox = target.getBBox();
  } catch (e) {
    _warn3("Some browsers won't measure invisible elements (like display:none or masks inside defs).");
  }
  var _ref = bbox || {
    x: 0,
    y: 0,
    width: 0,
    height: 0
  }, x = _ref.x, y = _ref.y, width = _ref.width, height = _ref.height;
  if ((!bbox || !width && !height) && _types[type]) {
    width = _getAttributeAsNumber(target, _types[type][0]);
    height = _getAttributeAsNumber(target, _types[type][1]);
    if (type !== "rect" && type !== "line") {
      width *= 2;
      height *= 2;
    }
    if (type === "line") {
      x = _getAttributeAsNumber(target, "x1");
      y = _getAttributeAsNumber(target, "y1");
      width = Math.abs(width - x);
      height = Math.abs(height - y);
    }
  }
  if (type === "path") {
    prevPoint = style.strokeDasharray;
    style.strokeDasharray = "none";
    length = target.getTotalLength() || 0;
    _round9(scaleX) !== _round9(scaleY) && !_warned && (_warned = 1) && _warn3("Warning: <path> length cannot be measured when vector-effect is non-scaling-stroke and the element isn't proportionally scaled.");
    length *= (scaleX + scaleY) / 2;
    style.strokeDasharray = prevPoint;
  } else if (type === "rect") {
    length = width * 2 * scaleX + height * 2 * scaleY;
  } else if (type === "line") {
    length = _getDistance(x, y, x + width, y + height, scaleX, scaleY);
  } else if (type === "polyline" || type === "polygon") {
    points = target.getAttribute("points").match(_numExp4) || [];
    type === "polygon" && points.push(points[0], points[1]);
    length = 0;
    for (i2 = 2; i2 < points.length; i2 += 2) {
      length += _getDistance(points[i2 - 2], points[i2 - 1], points[i2], points[i2 + 1], scaleX, scaleY) || 0;
    }
  } else if (type === "circle" || type === "ellipse") {
    rx = width / 2 * scaleX;
    ry = height / 2 * scaleY;
    length = Math.PI * (3 * (rx + ry) - _sqrt4((3 * rx + ry) * (rx + 3 * ry)));
  }
  return length || 0;
};
var _getPosition = function _getPosition2(target, length) {
  target = _toArray3(target)[0];
  if (!target) {
    return [0, 0];
  }
  length || (length = _getLength(target) + 1);
  var cs = _win3.getComputedStyle(target), dash = cs.strokeDasharray || "", offset = _parseNum(cs.strokeDashoffset), i2 = dash.indexOf(",");
  i2 < 0 && (i2 = dash.indexOf(" "));
  dash = i2 < 0 ? length : _parseNum(dash.substr(0, i2));
  dash > length && (dash = length);
  return [-offset || 0, dash - offset || 0];
};
var _initCore13 = function _initCore14() {
  if (_windowExists5()) {
    _doc4 = document;
    _win3 = window;
    _coreInitted8 = gsap8 = _getGSAP11();
    _toArray3 = gsap8.utils.toArray;
    _getStyleSaver3 = gsap8.core.getStyleSaver;
    _reverting4 = gsap8.core.reverting || function() {
    };
    _isEdge = ((_win3.navigator || {}).userAgent || "").indexOf("Edge") !== -1;
  }
};
var DrawSVGPlugin = {
  version: "3.15.0",
  name: "drawSVG",
  register: function register2(core) {
    gsap8 = core;
    _initCore13();
  },
  init: function init5(target, value, tween, index, targets) {
    if (!target.getBBox) {
      return false;
    }
    _coreInitted8 || _initCore13();
    var length = _getLength(target), start, end, cs;
    this.styles = _getStyleSaver3 && _getStyleSaver3(target, "strokeDashoffset,strokeDasharray,strokeMiterlimit");
    this.tween = tween;
    this._style = target.style;
    this._target = target;
    if (value + "" === "true") {
      value = "0 100%";
    } else if (!value) {
      value = "0 0";
    } else if ((value + "").indexOf(" ") === -1) {
      value = "0 " + value;
    }
    start = _getPosition(target, length);
    end = _parse(value, length, start[0]);
    this._length = _round9(length);
    this._dash = _round9(start[1] - start[0]);
    this._offset = _round9(-start[0]);
    this._dashPT = this.add(this, "_dash", this._dash, _round9(end[1] - end[0]), 0, 0, 0, 0, 0, 1);
    this._offsetPT = this.add(this, "_offset", this._offset, _round9(-end[0]), 0, 0, 0, 0, 0, 1);
    if (_isEdge) {
      cs = _win3.getComputedStyle(target);
      if (cs.strokeLinecap !== cs.strokeLinejoin) {
        end = _parseNum(cs.strokeMiterlimit);
        this.add(target.style, "strokeMiterlimit", end, end + 0.01);
      }
    }
    this._live = _hasNonScalingStroke(target) || ~(value + "").indexOf("live");
    this._nowrap = ~(value + "").indexOf("nowrap");
    this._props.push("drawSVG");
    return _bonusValidated4;
  },
  render: function render4(ratio, data) {
    if (data.tween._time || !_reverting4()) {
      var pt = data._pt, style = data._style, length, lengthRatio, dash, offset;
      if (pt) {
        if (data._live) {
          length = _getLength(data._target);
          if (length !== data._length) {
            lengthRatio = length / data._length;
            data._length = length;
            if (data._offsetPT) {
              data._offsetPT.s *= lengthRatio;
              data._offsetPT.c *= lengthRatio;
            }
            if (data._dashPT) {
              data._dashPT.s *= lengthRatio;
              data._dashPT.c *= lengthRatio;
            } else {
              data._dash *= lengthRatio;
            }
          }
        }
        while (pt) {
          pt.r(ratio, pt.d);
          pt = pt._next;
        }
        dash = data._dash || ratio && ratio !== 1 && 1e-4 || 0;
        length = data._length - dash + 0.1;
        offset = data._offset;
        dash && offset && dash + Math.abs(offset % data._length) > data._length - 0.05 && (offset += offset < 0 ? 5e-3 : -5e-3) && (length += 5e-3);
        style.strokeDashoffset = dash ? offset : offset + 1e-3;
        style.strokeDasharray = length < 0.1 ? "none" : dash ? dash + "px," + (data._nowrap ? 999999 : length) + "px" : "0px, 999999px";
      }
    } else {
      data.styles.revert();
    }
  },
  getLength: _getLength,
  getPosition: _getPosition
};
_getGSAP11() && gsap8.registerPlugin(DrawSVGPlugin);

// examples/figma-launch/node_modules/gsap/utils/matrix.js
var _doc5;
var _win4;
var _docElement2;
var _body;
var _divContainer;
var _svgContainer;
var _identityMatrix;
var _gEl;
var _transformProp2 = "transform";
var _transformOriginProp2 = _transformProp2 + "Origin";
var _hasOffsetBug;
var _setDoc = function _setDoc2(element) {
  var doc = element.ownerDocument || element;
  if (!(_transformProp2 in element.style) && "msTransform" in element.style) {
    _transformProp2 = "msTransform";
    _transformOriginProp2 = _transformProp2 + "Origin";
  }
  while (doc.parentNode && (doc = doc.parentNode)) {
  }
  _win4 = window;
  _identityMatrix = new Matrix2D();
  if (doc) {
    _doc5 = doc;
    _docElement2 = doc.documentElement;
    _body = doc.body;
    _gEl = _doc5.createElementNS("http://www.w3.org/2000/svg", "g");
    _gEl.style.transform = "none";
    var d1 = doc.createElement("div"), d2 = doc.createElement("div"), root = doc && (doc.body || doc.firstElementChild);
    if (root && root.appendChild) {
      root.appendChild(d1);
      d1.appendChild(d2);
      d1.style.position = "static";
      d1.style.transform = "translate3d(0,0,1px)";
      _hasOffsetBug = d2.offsetParent !== d1;
      root.removeChild(d1);
    }
  }
  return doc;
};
var _forceNonZeroScale = function _forceNonZeroScale2(e) {
  var a, cache;
  while (e && e !== _body) {
    cache = e._gsap;
    cache && cache.uncache && cache.get(e, "x");
    if (cache && !cache.scaleX && !cache.scaleY && cache.renderTransform) {
      cache.scaleX = cache.scaleY = 1e-4;
      cache.renderTransform(1, cache);
      a ? a.push(cache) : a = [cache];
    }
    e = e.parentNode;
  }
  return a;
};
var _svgTemps = [];
var _divTemps = [];
var _getDocScrollTop = function _getDocScrollTop2() {
  return _win4.pageYOffset || _doc5.scrollTop || _docElement2.scrollTop || _body.scrollTop || 0;
};
var _getDocScrollLeft = function _getDocScrollLeft2() {
  return _win4.pageXOffset || _doc5.scrollLeft || _docElement2.scrollLeft || _body.scrollLeft || 0;
};
var _svgOwner = function _svgOwner2(element) {
  return element.ownerSVGElement || ((element.tagName + "").toLowerCase() === "svg" ? element : null);
};
var _isFixed = function _isFixed2(element) {
  if (_win4.getComputedStyle(element).position === "fixed") {
    return true;
  }
  element = element.parentNode;
  if (element && element.nodeType === 1) {
    return _isFixed2(element);
  }
};
var _createSibling = function _createSibling2(element, i2) {
  if (element.parentNode && (_doc5 || _setDoc(element))) {
    var svg = _svgOwner(element), ns = svg ? svg.getAttribute("xmlns") || "http://www.w3.org/2000/svg" : "http://www.w3.org/1999/xhtml", type = svg ? i2 ? "rect" : "g" : "div", x = i2 !== 2 ? 0 : 100, y = i2 === 3 ? 100 : 0, css = {
      position: "absolute",
      display: "block",
      pointerEvents: "none",
      margin: "0",
      padding: "0"
    }, e = _doc5.createElementNS ? _doc5.createElementNS(ns.replace(/^https/, "http"), type) : _doc5.createElement(type);
    if (i2) {
      if (!svg) {
        if (!_divContainer) {
          _divContainer = _createSibling2(element);
          Object.assign(_divContainer.style, css);
        }
        Object.assign(e.style, css, {
          width: "0.1px",
          height: "0.1px",
          top: y + "px",
          left: x + "px"
        });
        _divContainer.appendChild(e);
      } else {
        _svgContainer || (_svgContainer = _createSibling2(element));
        e.setAttribute("width", 0.01);
        e.setAttribute("height", 0.01);
        e.setAttribute("transform", "translate(" + x + "," + y + ")");
        e.setAttribute("fill", "transparent");
        _svgContainer.appendChild(e);
      }
    }
    return e;
  }
  throw "Need document and parent.";
};
var _consolidate = function _consolidate2(m) {
  var c = new Matrix2D(), i2 = 0;
  for (; i2 < m.numberOfItems; i2++) {
    c.multiply(m.getItem(i2).matrix);
  }
  return c;
};
var _getCTM = function _getCTM2(svg) {
  var m = svg.getCTM(), transform;
  if (!m) {
    transform = svg.style[_transformProp2];
    svg.style[_transformProp2] = "none";
    svg.appendChild(_gEl);
    m = _gEl.getCTM();
    svg.removeChild(_gEl);
    transform ? svg.style[_transformProp2] = transform : svg.style.removeProperty(_transformProp2.replace(/([A-Z])/g, "-$1").toLowerCase());
  }
  return m || _identityMatrix.clone();
};
var _placeSiblings = function _placeSiblings2(element, adjustGOffset) {
  var svg = _svgOwner(element), isRootSVG = element === svg, siblings = svg ? _svgTemps : _divTemps, parent = element.parentNode, appendToEl = parent && !svg && parent.shadowRoot && parent.shadowRoot.appendChild ? parent.shadowRoot : parent, container, m, b, x, y, cs;
  if (element === _win4) {
    return element;
  }
  siblings.length || siblings.push(_createSibling(element, 1), _createSibling(element, 2), _createSibling(element, 3));
  container = svg ? _svgContainer : _divContainer;
  if (svg) {
    if (isRootSVG) {
      b = _getCTM(element);
      x = -b.e / b.a;
      y = -b.f / b.d;
      m = _identityMatrix;
    } else if (element.getBBox) {
      b = element.getBBox();
      m = element.transform ? element.transform.baseVal : {};
      m = !m.numberOfItems ? _identityMatrix : m.numberOfItems > 1 ? _consolidate(m) : m.getItem(0).matrix;
      x = m.a * b.x + m.c * b.y;
      y = m.b * b.x + m.d * b.y;
    } else {
      m = new Matrix2D();
      x = y = 0;
    }
    if (adjustGOffset && element.tagName.toLowerCase() === "g") {
      x = y = 0;
    }
    (isRootSVG || !element.getBoundingClientRect().width ? svg : parent).appendChild(container);
    container.setAttribute("transform", "matrix(" + m.a + "," + m.b + "," + m.c + "," + m.d + "," + (m.e + x) + "," + (m.f + y) + ")");
  } else {
    x = y = 0;
    if (_hasOffsetBug) {
      m = element.offsetParent;
      b = element;
      while (b && (b = b.parentNode) && b !== m && b.parentNode) {
        if ((_win4.getComputedStyle(b)[_transformProp2] + "").length > 4) {
          x = b.offsetLeft;
          y = b.offsetTop;
          b = 0;
        }
      }
    }
    cs = _win4.getComputedStyle(element);
    if (cs.position !== "absolute" && cs.position !== "fixed") {
      m = element.offsetParent;
      while (parent && parent !== m) {
        x += parent.scrollLeft || 0;
        y += parent.scrollTop || 0;
        parent = parent.parentNode;
      }
    }
    b = container.style;
    b.top = element.offsetTop - y + "px";
    b.left = element.offsetLeft - x + "px";
    b[_transformProp2] = cs[_transformProp2];
    b[_transformOriginProp2] = cs[_transformOriginProp2];
    b.position = cs.position === "fixed" ? "fixed" : "absolute";
    appendToEl.appendChild(container);
  }
  return container;
};
var _setMatrix = function _setMatrix2(m, a, b, c, d, e, f) {
  m.a = a;
  m.b = b;
  m.c = c;
  m.d = d;
  m.e = e;
  m.f = f;
  return m;
};
var Matrix2D = /* @__PURE__ */ (function() {
  function Matrix2D2(a, b, c, d, e, f) {
    if (a === void 0) {
      a = 1;
    }
    if (b === void 0) {
      b = 0;
    }
    if (c === void 0) {
      c = 0;
    }
    if (d === void 0) {
      d = 1;
    }
    if (e === void 0) {
      e = 0;
    }
    if (f === void 0) {
      f = 0;
    }
    _setMatrix(this, a, b, c, d, e, f);
  }
  var _proto = Matrix2D2.prototype;
  _proto.inverse = function inverse() {
    var a = this.a, b = this.b, c = this.c, d = this.d, e = this.e, f = this.f, determinant = a * d - b * c || 1e-10;
    return _setMatrix(this, d / determinant, -b / determinant, -c / determinant, a / determinant, (c * f - d * e) / determinant, -(a * f - b * e) / determinant);
  };
  _proto.multiply = function multiply(matrix) {
    var a = this.a, b = this.b, c = this.c, d = this.d, e = this.e, f = this.f, a2 = matrix.a, b2 = matrix.c, c2 = matrix.b, d2 = matrix.d, e2 = matrix.e, f2 = matrix.f;
    return _setMatrix(this, a2 * a + c2 * c, a2 * b + c2 * d, b2 * a + d2 * c, b2 * b + d2 * d, e + e2 * a + f2 * c, f + e2 * b + f2 * d);
  };
  _proto.clone = function clone() {
    return new Matrix2D2(this.a, this.b, this.c, this.d, this.e, this.f);
  };
  _proto.equals = function equals(matrix) {
    var a = this.a, b = this.b, c = this.c, d = this.d, e = this.e, f = this.f;
    return a === matrix.a && b === matrix.b && c === matrix.c && d === matrix.d && e === matrix.e && f === matrix.f;
  };
  _proto.apply = function apply(point, decoratee) {
    if (decoratee === void 0) {
      decoratee = {};
    }
    var x = point.x, y = point.y, a = this.a, b = this.b, c = this.c, d = this.d, e = this.e, f = this.f;
    decoratee.x = x * a + y * c + e || 0;
    decoratee.y = x * b + y * d + f || 0;
    return decoratee;
  };
  return Matrix2D2;
})();
function getGlobalMatrix(element, inverse, adjustGOffset, includeScrollInFixed) {
  if (!element || !element.parentNode || (_doc5 || _setDoc(element)).documentElement === element) {
    return new Matrix2D();
  }
  var zeroScales = _forceNonZeroScale(element), svg = _svgOwner(element), temps = svg ? _svgTemps : _divTemps, container = _placeSiblings(element, adjustGOffset), b1 = temps[0].getBoundingClientRect(), b2 = temps[1].getBoundingClientRect(), b3 = temps[2].getBoundingClientRect(), parent = container.parentNode, isFixed = !includeScrollInFixed && _isFixed(element), m = new Matrix2D((b2.left - b1.left) / 100, (b2.top - b1.top) / 100, (b3.left - b1.left) / 100, (b3.top - b1.top) / 100, b1.left + (isFixed ? 0 : _getDocScrollLeft()), b1.top + (isFixed ? 0 : _getDocScrollTop()));
  parent.removeChild(container);
  if (zeroScales) {
    b1 = zeroScales.length;
    while (b1--) {
      b2 = zeroScales[b1];
      b2.scaleX = b2.scaleY = 0;
      b2.renderTransform(1, b2);
    }
  }
  return inverse ? m.inverse() : m;
}

// examples/figma-launch/node_modules/gsap/MotionPathPlugin.js
var _xProps = "x,translateX,left,marginLeft,xPercent".split(",");
var _yProps = "y,translateY,top,marginTop,yPercent".split(",");
var _DEG2RAD3 = Math.PI / 180;
var gsap9;
var PropTween2;
var _getUnit;
var _toArray4;
var _getStyleSaver4;
var _reverting5;
var _getGSAP13 = function _getGSAP14() {
  return gsap9 || typeof window !== "undefined" && (gsap9 = window.gsap) && gsap9.registerPlugin && gsap9;
};
var _populateSegmentFromArray = function _populateSegmentFromArray2(segment, values, property, mode) {
  var l = values.length, si = mode === 2 ? 0 : mode, i2 = 0, v;
  for (; i2 < l; i2++) {
    segment[si] = v = parseFloat(values[i2][property]);
    mode === 2 && (segment[si + 1] = 0);
    si += 2;
  }
  return segment;
};
var _getPropNum = function _getPropNum2(target, prop, unit) {
  return parseFloat(target._gsap.get(target, prop, unit || "px")) || 0;
};
var _relativize = function _relativize2(segment) {
  var x = segment[0], y = segment[1], i2;
  for (i2 = 2; i2 < segment.length; i2 += 2) {
    x = segment[i2] += x;
    y = segment[i2 + 1] += y;
  }
};
var _segmentToRawPath = function _segmentToRawPath2(plugin, segment, target, x, y, slicer, vars, unitX, unitY) {
  if (vars.type === "cubic") {
    segment = [segment];
  } else {
    vars.fromCurrent !== false && segment.unshift(_getPropNum(target, x, unitX), y ? _getPropNum(target, y, unitY) : 0);
    vars.relative && _relativize(segment);
    var pointFunc = y ? pointsToSegment : flatPointsToSegment;
    segment = [pointFunc(segment, vars.curviness)];
  }
  segment = slicer(_align(segment, target, vars));
  _addDimensionalPropTween(plugin, target, x, segment, "x", unitX);
  y && _addDimensionalPropTween(plugin, target, y, segment, "y", unitY);
  return cacheRawPathMeasurements(segment, vars.resolution || (vars.curviness === 0 ? 20 : 12));
};
var _emptyFunc3 = function _emptyFunc4(v) {
  return v;
};
var _numExp5 = /[-+\.]*\d+\.?(?:e-|e\+)?\d*/g;
var _originToPoint = function _originToPoint2(element, origin, parentMatrix) {
  var m = getGlobalMatrix(element), x = 0, y = 0, svg;
  if ((element.tagName + "").toLowerCase() === "svg") {
    svg = element.viewBox.baseVal;
    svg.width || (svg = {
      width: +element.getAttribute("width"),
      height: +element.getAttribute("height")
    });
  } else {
    svg = origin && element.getBBox && element.getBBox();
  }
  if (origin && origin !== "auto") {
    x = origin.push ? origin[0] * (svg ? svg.width : element.offsetWidth || 0) : origin.x;
    y = origin.push ? origin[1] * (svg ? svg.height : element.offsetHeight || 0) : origin.y;
  }
  return parentMatrix.apply(x || y ? m.apply({
    x,
    y
  }) : {
    x: m.e,
    y: m.f
  });
};
var _getAlignMatrix = function _getAlignMatrix2(fromElement, toElement, fromOrigin, toOrigin) {
  var parentMatrix = getGlobalMatrix(fromElement.parentNode, true, true), m = parentMatrix.clone().multiply(getGlobalMatrix(toElement)), fromPoint = _originToPoint(fromElement, fromOrigin, parentMatrix), _originToPoint22 = _originToPoint(toElement, toOrigin, parentMatrix), x = _originToPoint22.x, y = _originToPoint22.y, p2;
  m.e = m.f = 0;
  if (toOrigin === "auto" && toElement.getTotalLength && toElement.tagName.toLowerCase() === "path") {
    p2 = toElement.getAttribute("d").match(_numExp5) || [];
    p2 = m.apply({
      x: +p2[0],
      y: +p2[1]
    });
    x += p2.x;
    y += p2.y;
  }
  if (p2) {
    p2 = m.apply(toElement.getBBox());
    x -= p2.x;
    y -= p2.y;
  }
  m.e = x - fromPoint.x;
  m.f = y - fromPoint.y;
  return m;
};
var _align = function _align2(rawPath, target, _ref) {
  var align = _ref.align, matrix = _ref.matrix, offsetX = _ref.offsetX, offsetY = _ref.offsetY, alignOrigin = _ref.alignOrigin;
  var x = rawPath[0][0], y = rawPath[0][1], curX = _getPropNum(target, "x"), curY = _getPropNum(target, "y"), alignTarget, m, p2;
  if (!rawPath || !rawPath.length) {
    return getRawPath("M0,0L0,0");
  }
  if (align) {
    if (align === "self" || (alignTarget = _toArray4(align)[0] || target) === target) {
      transformRawPath(rawPath, 1, 0, 0, 1, curX - x, curY - y);
    } else {
      if (alignOrigin && alignOrigin[2] !== false) {
        gsap9.set(target, {
          transformOrigin: alignOrigin[0] * 100 + "% " + alignOrigin[1] * 100 + "%"
        });
      } else {
        alignOrigin = [_getPropNum(target, "xPercent") / -100, _getPropNum(target, "yPercent") / -100];
      }
      m = _getAlignMatrix(target, alignTarget, alignOrigin, "auto");
      p2 = m.apply({
        x,
        y
      });
      transformRawPath(rawPath, m.a, m.b, m.c, m.d, curX + m.e - (p2.x - m.e), curY + m.f - (p2.y - m.f));
    }
  }
  if (matrix) {
    transformRawPath(rawPath, matrix.a, matrix.b, matrix.c, matrix.d, matrix.e, matrix.f);
  } else if (offsetX || offsetY) {
    transformRawPath(rawPath, 1, 0, 0, 1, offsetX || 0, offsetY || 0);
  }
  return rawPath;
};
var _addDimensionalPropTween = function _addDimensionalPropTween2(plugin, target, property, rawPath, pathProperty, forceUnit) {
  var cache = target._gsap, harness = cache.harness, alias = harness && harness.aliases && harness.aliases[property], prop = alias && alias.indexOf(",") < 0 ? alias : property, pt = plugin._pt = new PropTween2(plugin._pt, target, prop, 0, 0, _emptyFunc3, 0, cache.set(target, prop, plugin));
  pt.u = _getUnit(cache.get(target, prop, forceUnit)) || 0;
  pt.path = rawPath;
  pt.pp = pathProperty;
  plugin._props.push(prop);
};
var _sliceModifier = function _sliceModifier2(start, end) {
  return function(rawPath) {
    return start || end !== 1 ? sliceRawPath(rawPath, start, end) : rawPath;
  };
};
var MotionPathPlugin = {
  version: "3.15.0",
  name: "motionPath",
  register: function register3(core, Plugin, propTween) {
    gsap9 = core;
    _getUnit = gsap9.utils.getUnit;
    _toArray4 = gsap9.utils.toArray;
    _getStyleSaver4 = gsap9.core.getStyleSaver;
    _reverting5 = gsap9.core.reverting || function() {
    };
    PropTween2 = propTween;
  },
  init: function init6(target, vars, tween) {
    if (!gsap9) {
      console.warn("Please gsap.registerPlugin(MotionPathPlugin)");
      return false;
    }
    if (!(typeof vars === "object" && !vars.style) || !vars.path) {
      vars = {
        path: vars
      };
    }
    var rawPaths = [], _vars = vars, path = _vars.path, autoRotate = _vars.autoRotate, unitX = _vars.unitX, unitY = _vars.unitY, x = _vars.x, y = _vars.y, firstObj = path[0], slicer = _sliceModifier(vars.start, "end" in vars ? vars.end : 1), rawPath, p2;
    this.rawPaths = rawPaths;
    this.target = target;
    this.tween = tween;
    this.styles = _getStyleSaver4 && _getStyleSaver4(target, "transform");
    if (this.rotate = autoRotate || autoRotate === 0) {
      this.rOffset = parseFloat(autoRotate) || 0;
      this.radians = !!vars.useRadians;
      this.rProp = vars.rotation || "rotation";
      this.rSet = target._gsap.set(target, this.rProp, this);
      this.ru = _getUnit(target._gsap.get(target, this.rProp)) || 0;
    }
    if (Array.isArray(path) && !("closed" in path) && typeof firstObj !== "number") {
      for (p2 in firstObj) {
        if (!x && ~_xProps.indexOf(p2)) {
          x = p2;
        } else if (!y && ~_yProps.indexOf(p2)) {
          y = p2;
        }
      }
      if (x && y) {
        rawPaths.push(_segmentToRawPath(this, _populateSegmentFromArray(_populateSegmentFromArray([], path, x, 0), path, y, 1), target, x, y, slicer, vars, unitX || _getUnit(path[0][x]), unitY || _getUnit(path[0][y])));
      } else {
        x = y = 0;
      }
      for (p2 in firstObj) {
        p2 !== x && p2 !== y && rawPaths.push(_segmentToRawPath(this, _populateSegmentFromArray([], path, p2, 2), target, p2, 0, slicer, vars, _getUnit(path[0][p2])));
      }
    } else {
      rawPath = slicer(_align(getRawPath(vars.path), target, vars));
      cacheRawPathMeasurements(rawPath, vars.resolution);
      rawPaths.push(rawPath);
      _addDimensionalPropTween(this, target, vars.x || "x", rawPath, "x", vars.unitX || "px");
      _addDimensionalPropTween(this, target, vars.y || "y", rawPath, "y", vars.unitY || "px");
    }
    tween.vars.immediateRender && this.render(tween.progress(), this);
  },
  render: function render5(ratio, data) {
    var rawPaths = data.rawPaths, i2 = rawPaths.length, pt = data._pt;
    if (data.tween._time || !_reverting5()) {
      if (ratio > 1) {
        ratio = 1;
      } else if (ratio < 0) {
        ratio = 0;
      }
      while (i2--) {
        getPositionOnPath(rawPaths[i2], ratio, !i2 && data.rotate, rawPaths[i2]);
      }
      while (pt) {
        pt.set(pt.t, pt.p, pt.path[pt.pp] + pt.u, pt.d, ratio);
        pt = pt._next;
      }
      data.rotate && data.rSet(data.target, data.rProp, rawPaths[0].angle * (data.radians ? _DEG2RAD3 : 1) + data.rOffset + data.ru, data, ratio);
    } else {
      data.styles.revert();
    }
  },
  getLength: function getLength(path) {
    return cacheRawPathMeasurements(getRawPath(path)).totalLength;
  },
  sliceRawPath,
  getRawPath,
  pointsToSegment,
  stringToRawPath,
  rawPathToString,
  transformRawPath,
  getGlobalMatrix,
  getPositionOnPath,
  cacheRawPathMeasurements,
  convertToPath: function convertToPath3(targets, swap) {
    return _toArray4(targets).map(function(target) {
      return convertToPath(target, swap !== false);
    });
  },
  convertCoordinates: function convertCoordinates(fromElement, toElement, point) {
    var m = getGlobalMatrix(toElement, true, true).multiply(getGlobalMatrix(fromElement));
    return point ? m.apply(point) : m;
  },
  getAlignMatrix: _getAlignMatrix,
  getRelativePosition: function getRelativePosition(fromElement, toElement, fromOrigin, toOrigin) {
    var m = _getAlignMatrix(fromElement, toElement, fromOrigin, toOrigin);
    return {
      x: m.e,
      y: m.f
    };
  },
  arrayToRawPath: function arrayToRawPath(value, vars) {
    vars = vars || {};
    var segment = _populateSegmentFromArray(_populateSegmentFromArray([], value, vars.x || "x", 0), value, vars.y || "y", 1);
    vars.relative && _relativize(segment);
    return [vars.type === "cubic" ? segment : pointsToSegment(segment, vars.curviness)];
  }
};
_getGSAP13() && gsap9.registerPlugin(MotionPathPlugin);

// examples/figma-launch/node_modules/gsap/utils/strings.js
var _trimExp = /(?:^\s+|\s+$)/g;
var emojiExp = /([\uD800-\uDBFF][\uDC00-\uDFFF](?:[\u200D\uFE0F][\uD800-\uDBFF][\uDC00-\uDFFF]){2,}|\uD83D\uDC69(?:\u200D(?:(?:\uD83D\uDC69\u200D)?\uD83D\uDC67|(?:\uD83D\uDC69\u200D)?\uD83D\uDC66)|\uD83C[\uDFFB-\uDFFF])|\uD83D\uDC69\u200D(?:\uD83D\uDC69\u200D)?\uD83D\uDC66\u200D\uD83D\uDC66|\uD83D\uDC69\u200D(?:\uD83D\uDC69\u200D)?\uD83D\uDC67\u200D(?:\uD83D[\uDC66\uDC67])|\uD83C\uDFF3\uFE0F\u200D\uD83C\uDF08|(?:\uD83C[\uDFC3\uDFC4\uDFCA]|\uD83D[\uDC6E\uDC71\uDC73\uDC77\uDC81\uDC82\uDC86\uDC87\uDE45-\uDE47\uDE4B\uDE4D\uDE4E\uDEA3\uDEB4-\uDEB6]|\uD83E[\uDD26\uDD37-\uDD39\uDD3D\uDD3E\uDDD6-\uDDDD])(?:\uD83C[\uDFFB-\uDFFF])\u200D[\u2642\u2640]\uFE0F|\uD83D\uDC69(?:\uD83C[\uDFFB-\uDFFF])\u200D(?:\uD83C[\uDF3E\uDF73\uDF93\uDFA4\uDFA8\uDFEB\uDFED]|\uD83D[\uDCBB\uDD27\uDCBC\uDD2C\uDE80\uDE92])|(?:\uD83C[\uDFC3\uDFC4\uDFCA]|\uD83D[\uDC6E\uDC6F\uDC71\uDC73\uDC77\uDC81\uDC82\uDC86\uDC87\uDE45-\uDE47\uDE4B\uDE4D\uDE4E\uDEA3\uDEB4-\uDEB6]|\uD83E[\uDD26\uDD37-\uDD39\uDD3C-\uDD3E\uDDD6-\uDDDF])\u200D[\u2640\u2642]\uFE0F|\uD83C\uDDFD\uD83C\uDDF0|\uD83C\uDDF6\uD83C\uDDE6|\uD83C\uDDF4\uD83C\uDDF2|\uD83C\uDDE9(?:\uD83C[\uDDEA\uDDEC\uDDEF\uDDF0\uDDF2\uDDF4\uDDFF])|\uD83C\uDDF7(?:\uD83C[\uDDEA\uDDF4\uDDF8\uDDFA\uDDFC])|\uD83C\uDDE8(?:\uD83C[\uDDE6\uDDE8\uDDE9\uDDEB-\uDDEE\uDDF0-\uDDF5\uDDF7\uDDFA-\uDDFF])|(?:\u26F9|\uD83C[\uDFCC\uDFCB]|\uD83D\uDD75)(?:\uFE0F\u200D[\u2640\u2642]|(?:\uD83C[\uDFFB-\uDFFF])\u200D[\u2640\u2642])\uFE0F|(?:\uD83D\uDC41\uFE0F\u200D\uD83D\uDDE8|\uD83D\uDC69(?:\uD83C[\uDFFB-\uDFFF])\u200D[\u2695\u2696\u2708]|\uD83D\uDC69\u200D[\u2695\u2696\u2708]|\uD83D\uDC68(?:(?:\uD83C[\uDFFB-\uDFFF])\u200D[\u2695\u2696\u2708]|\u200D[\u2695\u2696\u2708]))\uFE0F|\uD83C\uDDF2(?:\uD83C[\uDDE6\uDDE8-\uDDED\uDDF0-\uDDFF])|\uD83D\uDC69\u200D(?:\uD83C[\uDF3E\uDF73\uDF93\uDFA4\uDFA8\uDFEB\uDFED]|\uD83D[\uDCBB\uDCBC\uDD27\uDD2C\uDE80\uDE92]|\u2764\uFE0F\u200D(?:\uD83D\uDC8B\u200D(?:\uD83D[\uDC68\uDC69])|\uD83D[\uDC68\uDC69]))|\uD83C\uDDF1(?:\uD83C[\uDDE6-\uDDE8\uDDEE\uDDF0\uDDF7-\uDDFB\uDDFE])|\uD83C\uDDEF(?:\uD83C[\uDDEA\uDDF2\uDDF4\uDDF5])|\uD83C\uDDED(?:\uD83C[\uDDF0\uDDF2\uDDF3\uDDF7\uDDF9\uDDFA])|\uD83C\uDDEB(?:\uD83C[\uDDEE-\uDDF0\uDDF2\uDDF4\uDDF7])|[#\*0-9]\uFE0F\u20E3|\uD83C\uDDE7(?:\uD83C[\uDDE6\uDDE7\uDDE9-\uDDEF\uDDF1-\uDDF4\uDDF6-\uDDF9\uDDFB\uDDFC\uDDFE\uDDFF])|\uD83C\uDDE6(?:\uD83C[\uDDE8-\uDDEC\uDDEE\uDDF1\uDDF2\uDDF4\uDDF6-\uDDFA\uDDFC\uDDFD\uDDFF])|\uD83C\uDDFF(?:\uD83C[\uDDE6\uDDF2\uDDFC])|\uD83C\uDDF5(?:\uD83C[\uDDE6\uDDEA-\uDDED\uDDF0-\uDDF3\uDDF7-\uDDF9\uDDFC\uDDFE])|\uD83C\uDDFB(?:\uD83C[\uDDE6\uDDE8\uDDEA\uDDEC\uDDEE\uDDF3\uDDFA])|\uD83C\uDDF3(?:\uD83C[\uDDE6\uDDE8\uDDEA-\uDDEC\uDDEE\uDDF1\uDDF4\uDDF5\uDDF7\uDDFA\uDDFF])|\uD83C\uDFF4\uDB40\uDC67\uDB40\uDC62(?:\uDB40\uDC77\uDB40\uDC6C\uDB40\uDC73|\uDB40\uDC73\uDB40\uDC63\uDB40\uDC74|\uDB40\uDC65\uDB40\uDC6E\uDB40\uDC67)\uDB40\uDC7F|\uD83D\uDC68(?:\u200D(?:\u2764\uFE0F\u200D(?:\uD83D\uDC8B\u200D)?\uD83D\uDC68|(?:(?:\uD83D[\uDC68\uDC69])\u200D)?\uD83D\uDC66\u200D\uD83D\uDC66|(?:(?:\uD83D[\uDC68\uDC69])\u200D)?\uD83D\uDC67\u200D(?:\uD83D[\uDC66\uDC67])|\uD83C[\uDF3E\uDF73\uDF93\uDFA4\uDFA8\uDFEB\uDFED]|\uD83D[\uDCBB\uDCBC\uDD27\uDD2C\uDE80\uDE92])|(?:\uD83C[\uDFFB-\uDFFF])\u200D(?:\uD83C[\uDF3E\uDF73\uDF93\uDFA4\uDFA8\uDFEB\uDFED]|\uD83D[\uDCBB\uDCBC\uDD27\uDD2C\uDE80\uDE92]))|\uD83C\uDDF8(?:\uD83C[\uDDE6-\uDDEA\uDDEC-\uDDF4\uDDF7-\uDDF9\uDDFB\uDDFD-\uDDFF])|\uD83C\uDDF0(?:\uD83C[\uDDEA\uDDEC-\uDDEE\uDDF2\uDDF3\uDDF5\uDDF7\uDDFC\uDDFE\uDDFF])|\uD83C\uDDFE(?:\uD83C[\uDDEA\uDDF9])|\uD83C\uDDEE(?:\uD83C[\uDDE8-\uDDEA\uDDF1-\uDDF4\uDDF6-\uDDF9])|\uD83C\uDDF9(?:\uD83C[\uDDE6\uDDE8\uDDE9\uDDEB-\uDDED\uDDEF-\uDDF4\uDDF7\uDDF9\uDDFB\uDDFC\uDDFF])|\uD83C\uDDEC(?:\uD83C[\uDDE6\uDDE7\uDDE9-\uDDEE\uDDF1-\uDDF3\uDDF5-\uDDFA\uDDFC\uDDFE])|\uD83C\uDDFA(?:\uD83C[\uDDE6\uDDEC\uDDF2\uDDF3\uDDF8\uDDFE\uDDFF])|\uD83C\uDDEA(?:\uD83C[\uDDE6\uDDE8\uDDEA\uDDEC\uDDED\uDDF7-\uDDFA])|\uD83C\uDDFC(?:\uD83C[\uDDEB\uDDF8])|(?:\u26F9|\uD83C[\uDFCB\uDFCC]|\uD83D\uDD75)(?:\uD83C[\uDFFB-\uDFFF])|(?:\uD83C[\uDFC3\uDFC4\uDFCA]|\uD83D[\uDC6E\uDC71\uDC73\uDC77\uDC81\uDC82\uDC86\uDC87\uDE45-\uDE47\uDE4B\uDE4D\uDE4E\uDEA3\uDEB4-\uDEB6]|\uD83E[\uDD26\uDD37-\uDD39\uDD3D\uDD3E\uDDD6-\uDDDD])(?:\uD83C[\uDFFB-\uDFFF])|(?:[\u261D\u270A-\u270D]|\uD83C[\uDF85\uDFC2\uDFC7]|\uD83D[\uDC42\uDC43\uDC46-\uDC50\uDC66\uDC67\uDC70\uDC72\uDC74-\uDC76\uDC78\uDC7C\uDC83\uDC85\uDCAA\uDD74\uDD7A\uDD90\uDD95\uDD96\uDE4C\uDE4F\uDEC0\uDECC]|\uD83E[\uDD18-\uDD1C\uDD1E\uDD1F\uDD30-\uDD36\uDDD1-\uDDD5])(?:\uD83C[\uDFFB-\uDFFF])|\uD83D\uDC68(?:\u200D(?:(?:(?:\uD83D[\uDC68\uDC69])\u200D)?\uD83D\uDC67|(?:(?:\uD83D[\uDC68\uDC69])\u200D)?\uD83D\uDC66)|\uD83C[\uDFFB-\uDFFF])|(?:[\u261D\u26F9\u270A-\u270D]|\uD83C[\uDF85\uDFC2-\uDFC4\uDFC7\uDFCA-\uDFCC]|\uD83D[\uDC42\uDC43\uDC46-\uDC50\uDC66-\uDC69\uDC6E\uDC70-\uDC78\uDC7C\uDC81-\uDC83\uDC85-\uDC87\uDCAA\uDD74\uDD75\uDD7A\uDD90\uDD95\uDD96\uDE45-\uDE47\uDE4B-\uDE4F\uDEA3\uDEB4-\uDEB6\uDEC0\uDECC]|\uD83E[\uDD18-\uDD1C\uDD1E\uDD1F\uDD26\uDD30-\uDD39\uDD3D\uDD3E\uDDD1-\uDDDD])(?:\uD83C[\uDFFB-\uDFFF])?|(?:[\u231A\u231B\u23E9-\u23EC\u23F0\u23F3\u25FD\u25FE\u2614\u2615\u2648-\u2653\u267F\u2693\u26A1\u26AA\u26AB\u26BD\u26BE\u26C4\u26C5\u26CE\u26D4\u26EA\u26F2\u26F3\u26F5\u26FA\u26FD\u2705\u270A\u270B\u2728\u274C\u274E\u2753-\u2755\u2757\u2795-\u2797\u27B0\u27BF\u2B1B\u2B1C\u2B50\u2B55]|\uD83C[\uDC04\uDCCF\uDD8E\uDD91-\uDD9A\uDDE6-\uDDFF\uDE01\uDE1A\uDE2F\uDE32-\uDE36\uDE38-\uDE3A\uDE50\uDE51\uDF00-\uDF20\uDF2D-\uDF35\uDF37-\uDF7C\uDF7E-\uDF93\uDFA0-\uDFCA\uDFCF-\uDFD3\uDFE0-\uDFF0\uDFF4\uDFF8-\uDFFF]|\uD83D[\uDC00-\uDC3E\uDC40\uDC42-\uDCFC\uDCFF-\uDD3D\uDD4B-\uDD4E\uDD50-\uDD67\uDD7A\uDD95\uDD96\uDDA4\uDDFB-\uDE4F\uDE80-\uDEC5\uDECC\uDED0-\uDED2\uDEEB\uDEEC\uDEF4-\uDEF8]|\uD83E[\uDD10-\uDD3A\uDD3C-\uDD3E\uDD40-\uDD45\uDD47-\uDD4C\uDD50-\uDD6B\uDD80-\uDD97\uDDC0\uDDD0-\uDDE6])|(?:[#\*0-9\xA9\xAE\u203C\u2049\u2122\u2139\u2194-\u2199\u21A9\u21AA\u231A\u231B\u2328\u23CF\u23E9-\u23F3\u23F8-\u23FA\u24C2\u25AA\u25AB\u25B6\u25C0\u25FB-\u25FE\u2600-\u2604\u260E\u2611\u2614\u2615\u2618\u261D\u2620\u2622\u2623\u2626\u262A\u262E\u262F\u2638-\u263A\u2640\u2642\u2648-\u2653\u2660\u2663\u2665\u2666\u2668\u267B\u267F\u2692-\u2697\u2699\u269B\u269C\u26A0\u26A1\u26AA\u26AB\u26B0\u26B1\u26BD\u26BE\u26C4\u26C5\u26C8\u26CE\u26CF\u26D1\u26D3\u26D4\u26E9\u26EA\u26F0-\u26F5\u26F7-\u26FA\u26FD\u2702\u2705\u2708-\u270D\u270F\u2712\u2714\u2716\u271D\u2721\u2728\u2733\u2734\u2744\u2747\u274C\u274E\u2753-\u2755\u2757\u2763\u2764\u2795-\u2797\u27A1\u27B0\u27BF\u2934\u2935\u2B05-\u2B07\u2B1B\u2B1C\u2B50\u2B55\u3030\u303D\u3297\u3299]|\uD83C[\uDC04\uDCCF\uDD70\uDD71\uDD7E\uDD7F\uDD8E\uDD91-\uDD9A\uDDE6-\uDDFF\uDE01\uDE02\uDE1A\uDE2F\uDE32-\uDE3A\uDE50\uDE51\uDF00-\uDF21\uDF24-\uDF93\uDF96\uDF97\uDF99-\uDF9B\uDF9E-\uDFF0\uDFF3-\uDFF5\uDFF7-\uDFFF]|\uD83D[\uDC00-\uDCFD\uDCFF-\uDD3D\uDD49-\uDD4E\uDD50-\uDD67\uDD6F\uDD70\uDD73-\uDD7A\uDD87\uDD8A-\uDD8D\uDD90\uDD95\uDD96\uDDA4\uDDA5\uDDA8\uDDB1\uDDB2\uDDBC\uDDC2-\uDDC4\uDDD1-\uDDD3\uDDDC-\uDDDE\uDDE1\uDDE3\uDDE8\uDDEF\uDDF3\uDDFA-\uDE4F\uDE80-\uDEC5\uDECB-\uDED2\uDEE0-\uDEE5\uDEE9\uDEEB\uDEEC\uDEF0\uDEF3-\uDEF8]|\uD83E[\uDD10-\uDD3A\uDD3C-\uDD3E\uDD40-\uDD45\uDD47-\uDD4C\uDD50-\uDD6B\uDD80-\uDD97\uDDC0\uDDD0-\uDDE6])\uFE0F)/;
function getText(e) {
  var type = e.nodeType, result = "";
  if (type === 1 || type === 9 || type === 11) {
    if (typeof e.textContent === "string") {
      return e.textContent;
    } else {
      for (e = e.firstChild; e; e = e.nextSibling) {
        result += getText(e);
      }
    }
  } else if (type === 3 || type === 4) {
    return e.nodeValue;
  }
  return result;
}
function splitInnerHTML(element, delimiter, trim, preserveSpaces, unescapedCharCodes) {
  var node = element.firstChild, result = [], s;
  while (node) {
    if (node.nodeType === 3) {
      s = (node.nodeValue + "").replace(/^\n+/g, "");
      if (!preserveSpaces) {
        s = s.replace(/\s+/g, " ");
      }
      result.push.apply(result, emojiSafeSplit(s, delimiter, trim, preserveSpaces, unescapedCharCodes));
    } else if ((node.nodeName + "").toLowerCase() === "br") {
      result[result.length - 1] += "<br>";
    } else {
      result.push(node.outerHTML);
    }
    node = node.nextSibling;
  }
  if (!unescapedCharCodes) {
    s = result.length;
    while (s--) {
      result[s] === "&" && result.splice(s, 1, "&amp;");
    }
  }
  return result;
}
function emojiSafeSplit(text, delimiter, trim, preserveSpaces, unescapedCharCodes) {
  text += "";
  trim && (text = text.trim ? text.trim() : text.replace(_trimExp, ""));
  if (delimiter && delimiter !== "") {
    return text.replace(/>/g, "&gt;").replace(/</g, "&lt;").split(delimiter);
  }
  var result = [], l = text.length, i2 = 0, j, character;
  for (; i2 < l; i2++) {
    character = text.charAt(i2);
    if (character.charCodeAt(0) >= 55296 && character.charCodeAt(0) <= 56319 || text.charCodeAt(i2 + 1) >= 65024 && text.charCodeAt(i2 + 1) <= 65039) {
      j = ((text.substr(i2, 12).split(emojiExp) || [])[1] || "").length || 2;
      character = text.substr(i2, j);
      result.emoji = 1;
      i2 += j - 1;
    }
    result.push(unescapedCharCodes ? character : character === ">" ? "&gt;" : character === "<" ? "&lt;" : preserveSpaces && character === " " && (text.charAt(i2 - 1) === " " || text.charAt(i2 + 1) === " ") ? "&nbsp;" : character);
  }
  return result;
}

// examples/figma-launch/node_modules/gsap/ScrambleTextPlugin.js
var CharSet = /* @__PURE__ */ (function() {
  function CharSet2(chars) {
    this.chars = emojiSafeSplit(chars);
    this.sets = [];
    this.length = 50;
    for (var i2 = 0; i2 < 20; i2++) {
      this.sets[i2] = _scrambleText(80, this.chars);
    }
  }
  var _proto = CharSet2.prototype;
  _proto.grow = function grow(newLength) {
    for (var i2 = 0; i2 < 20; i2++) {
      this.sets[i2] += _scrambleText(newLength - this.length, this.chars);
    }
    this.length = newLength;
  };
  return CharSet2;
})();
var gsap10;
var _coreInitted9;
var _getGSAP15 = function _getGSAP16() {
  return gsap10 || typeof window !== "undefined" && (gsap10 = window.gsap) && gsap10.registerPlugin && gsap10;
};
var _bonusValidated5 = 1;
var _spacesExp = /\s+/g;
var _scrambleText = function _scrambleText2(length, chars) {
  var l = chars.length, s = "";
  while (--length > -1) {
    s += chars[~~(Math.random() * l)];
  }
  return s;
};
var _upper = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
var _lower = _upper.toLowerCase();
var _charsLookup = {
  upperCase: new CharSet(_upper),
  lowerCase: new CharSet(_lower),
  upperAndLowerCase: new CharSet(_upper + _lower)
};
var _initCore15 = function _initCore16() {
  _coreInitted9 = gsap10 = _getGSAP15();
};
var ScrambleTextPlugin = {
  version: "3.15.0",
  name: "scrambleText",
  register: function register4(core, Plugin, propTween) {
    gsap10 = core;
    _initCore15();
  },
  init: function init7(target, value, tween, index, targets) {
    _coreInitted9 || _initCore15();
    this.prop = "innerHTML" in target ? "innerHTML" : "textContent" in target ? "textContent" : 0;
    if (!this.prop) {
      return;
    }
    this.target = target;
    if (typeof value !== "object") {
      value = {
        text: value
      };
    }
    var text = value.text || value.value || "", trim = value.trim !== false, data = this, delim, maxLength, charset, splitByChars;
    data.delimiter = delim = value.delimiter || "";
    data.original = emojiSafeSplit(getText(target).replace(_spacesExp, " ").split("&nbsp;").join(""), delim, trim);
    if (text === "{original}" || text === true || text == null) {
      text = data.original.join(delim);
    }
    data.text = emojiSafeSplit((text || "").replace(_spacesExp, " "), delim, trim);
    data.hasClass = !!(value.newClass || value.oldClass);
    data.newClass = value.newClass;
    data.oldClass = value.oldClass;
    splitByChars = delim === "";
    data.textHasEmoji = splitByChars && !!data.text.emoji;
    data.charsHaveEmoji = !!value.chars && !!emojiSafeSplit(value.chars).emoji;
    data.length = splitByChars ? data.original.length : data.original.join(delim).length;
    data.lengthDif = (splitByChars ? data.text.length : data.text.join(delim).length) - data.length;
    data.fillChar = value.fillChar || value.chars && ~value.chars.indexOf(" ") ? "&nbsp;" : "";
    data.charSet = charset = _charsLookup[value.chars || "upperCase"] || new CharSet(value.chars);
    data.speed = 0.05 / (value.speed || 1);
    data.prevScrambleTime = 0;
    data.setIndex = Math.random() * 20 | 0;
    maxLength = data.length + Math.max(data.lengthDif, 0);
    if (maxLength > charset.length) {
      charset.grow(maxLength);
    }
    data.chars = charset.sets[data.setIndex];
    data.revealDelay = value.revealDelay || 0;
    data.tweenLength = value.tweenLength !== false;
    data.tween = tween;
    data.rightToLeft = !!value.rightToLeft;
    data._props.push("scrambleText", "text");
    return _bonusValidated5;
  },
  render: function render6(ratio, data) {
    var target = data.target, prop = data.prop, text = data.text, delimiter = data.delimiter, tween = data.tween, prevScrambleTime = data.prevScrambleTime, revealDelay = data.revealDelay, setIndex = data.setIndex, chars = data.chars, charSet = data.charSet, length = data.length, textHasEmoji = data.textHasEmoji, charsHaveEmoji = data.charsHaveEmoji, lengthDif = data.lengthDif, tweenLength = data.tweenLength, oldClass = data.oldClass, newClass = data.newClass, rightToLeft = data.rightToLeft, fillChar = data.fillChar, speed = data.speed, original = data.original, hasClass = data.hasClass, l = text.length, time = tween._time, timeDif = time - prevScrambleTime, i2, i22, startText, endText, applyNew, applyOld, str, startClass, endClass, position, r;
    if (revealDelay) {
      if (tween._from) {
        time = tween._dur - time;
      }
      ratio = time === 0 ? 0 : time < revealDelay ? 1e-6 : time === tween._dur ? 1 : tween._ease((time - revealDelay) / (tween._dur - revealDelay));
    }
    if (ratio < 0) {
      ratio = 0;
    } else if (ratio > 1) {
      ratio = 1;
    }
    if (rightToLeft) {
      ratio = 1 - ratio;
    }
    i2 = ~~(ratio * l + 0.5);
    if (ratio) {
      if (timeDif > speed || timeDif < -speed) {
        data.setIndex = setIndex = (setIndex + (Math.random() * 19 | 0)) % 20;
        data.chars = charSet.sets[setIndex];
        data.prevScrambleTime += timeDif;
      }
      endText = chars;
    } else {
      endText = original.join(delimiter);
    }
    r = tween._from ? ratio : 1 - ratio;
    position = length + (tweenLength ? tween._from ? r * r * r : 1 - r * r * r : 1) * lengthDif;
    if (rightToLeft) {
      if (ratio === 1 && (tween._from || tween.data === "isFromStart")) {
        startText = "";
        endText = original.join(delimiter);
      } else {
        str = text.slice(i2).join(delimiter);
        if (charsHaveEmoji) {
          startText = emojiSafeSplit(endText).slice(0, position - (textHasEmoji ? emojiSafeSplit(str) : str).length + 0.5 | 0).join("");
        } else {
          startText = endText.substr(0, position - (textHasEmoji ? emojiSafeSplit(str) : str).length + 0.5 | 0);
        }
        endText = str;
      }
    } else {
      startText = text.slice(0, i2).join(delimiter);
      i22 = (textHasEmoji ? emojiSafeSplit(startText) : startText).length;
      if (charsHaveEmoji) {
        endText = emojiSafeSplit(endText).slice(i22, position + 0.5 | 0).join("");
      } else {
        endText = endText.substr(i22, position - i22 + 0.5 | 0);
      }
    }
    if (hasClass) {
      startClass = rightToLeft ? oldClass : newClass;
      endClass = rightToLeft ? newClass : oldClass;
      applyNew = startClass && i2 !== 0;
      applyOld = endClass && i2 !== l;
      str = (applyNew ? "<span class='" + startClass + "'>" : "") + startText + (applyNew ? "</span>" : "") + (applyOld ? "<span class='" + endClass + "'>" : "") + delimiter + endText + (applyOld ? "</span>" : "");
    } else {
      str = startText + delimiter + endText;
    }
    target[prop] = fillChar === "&nbsp;" && ~str.indexOf("  ") ? str.split("  ").join("&nbsp;&nbsp;") : str;
  }
};
ScrambleTextPlugin.emojiSafeSplit = emojiSafeSplit;
ScrambleTextPlugin.getText = getText;
_getGSAP15() && gsap10.registerPlugin(ScrambleTextPlugin);

// examples/figma-launch/node_modules/gsap/TextPlugin.js
var gsap11;
var _tempDiv2;
var _getGSAP17 = function _getGSAP18() {
  return gsap11 || typeof window !== "undefined" && (gsap11 = window.gsap) && gsap11.registerPlugin && gsap11;
};
var TextPlugin = {
  version: "3.15.0",
  name: "text",
  init: function init8(target, value, tween) {
    typeof value !== "object" && (value = {
      value
    });
    var i2 = target.nodeName.toUpperCase(), data = this, _value = value, newClass = _value.newClass, oldClass = _value.oldClass, preserveSpaces = _value.preserveSpaces, rtl = _value.rtl, delimiter = data.delimiter = value.delimiter || "", fillChar = data.fillChar = value.fillChar || (value.padSpace ? "&nbsp;" : ""), _short, text, original, j, condensedText, condensedOriginal, aggregate, s;
    data.svg = target.getBBox && (i2 === "TEXT" || i2 === "TSPAN");
    if (!("innerHTML" in target) && !data.svg) {
      return false;
    }
    data.target = target;
    if (!("value" in value)) {
      data.text = data.original = [""];
      return;
    }
    original = splitInnerHTML(target, delimiter, false, preserveSpaces, data.svg);
    _tempDiv2 || (_tempDiv2 = document.createElement("div"));
    _tempDiv2.innerHTML = value.value;
    text = splitInnerHTML(_tempDiv2, delimiter, false, preserveSpaces, data.svg);
    data.from = tween._from;
    if ((data.from || rtl) && !(rtl && data.from)) {
      i2 = original;
      original = text;
      text = i2;
    }
    data.hasClass = !!(newClass || oldClass);
    data.newClass = rtl ? oldClass : newClass;
    data.oldClass = rtl ? newClass : oldClass;
    i2 = original.length - text.length;
    _short = i2 < 0 ? original : text;
    if (i2 < 0) {
      i2 = -i2;
    }
    while (--i2 > -1) {
      _short.push(fillChar);
    }
    if (value.type === "diff") {
      j = 0;
      condensedText = [];
      condensedOriginal = [];
      aggregate = "";
      for (i2 = 0; i2 < text.length; i2++) {
        s = text[i2];
        if (s === original[i2]) {
          aggregate += s;
        } else {
          condensedText[j] = aggregate + s;
          condensedOriginal[j++] = aggregate + original[i2];
          aggregate = "";
        }
      }
      text = condensedText;
      original = condensedOriginal;
      if (aggregate) {
        text.push(aggregate);
        original.push(aggregate);
      }
    }
    value.speed && tween.duration(Math.min(0.05 / value.speed * _short.length, value.maxDuration || 9999));
    data.rtl = rtl;
    data.original = original;
    data.text = text;
    data._props.push("text");
  },
  render: function render7(ratio, data) {
    if (ratio > 1) {
      ratio = 1;
    } else if (ratio < 0) {
      ratio = 0;
    }
    if (data.from) {
      ratio = 1 - ratio;
    }
    var text = data.text, hasClass = data.hasClass, newClass = data.newClass, oldClass = data.oldClass, delimiter = data.delimiter, target = data.target, fillChar = data.fillChar, original = data.original, rtl = data.rtl, l = text.length, i2 = (rtl ? 1 - ratio : ratio) * l + 0.5 | 0, applyNew, applyOld, str;
    if (hasClass && ratio) {
      applyNew = newClass && i2;
      applyOld = oldClass && i2 !== l;
      str = (applyNew ? "<span class='" + newClass + "'>" : "") + text.slice(0, i2).join(delimiter) + (applyNew ? "</span>" : "") + (applyOld ? "<span class='" + oldClass + "'>" : "") + delimiter + original.slice(i2).join(delimiter) + (applyOld ? "</span>" : "");
    } else {
      str = text.slice(0, i2).join(delimiter) + delimiter + original.slice(i2).join(delimiter);
    }
    if (data.svg) {
      target.textContent = str;
    } else {
      target.innerHTML = fillChar === "&nbsp;" && ~str.indexOf("  ") ? str.split("  ").join("&nbsp;&nbsp;") : str;
    }
  }
};
TextPlugin.splitInnerHTML = splitInnerHTML;
TextPlugin.emojiSafeSplit = emojiSafeSplit;
TextPlugin.getText = getText;
_getGSAP17() && gsap11.registerPlugin(TextPlugin);

// examples/figma-launch/node_modules/gsap/Flip.js
var _id = 1;
var _toArray5;
var gsap12;
var _batch;
var _batchAction;
var _body2;
var _closestTenth;
var _getStyleSaver5;
var _forEachBatch = function _forEachBatch2(batch, name) {
  return batch.actions.forEach(function(a) {
    return a.vars[name] && a.vars[name](a);
  });
};
var _batchLookup = {};
var _RAD2DEG3 = 180 / Math.PI;
var _DEG2RAD4 = Math.PI / 180;
var _emptyObj = {};
var _dashedNameLookup = {};
var _memoizedRemoveProps = {};
var _listToArray = function _listToArray2(list) {
  return typeof list === "string" ? list.split(" ").join("").split(",") : list;
};
var _callbacks = _listToArray("onStart,onUpdate,onComplete,onReverseComplete,onInterrupt");
var _removeProps = _listToArray("transform,transformOrigin,width,height,position,top,left,opacity,zIndex,maxWidth,maxHeight,minWidth,minHeight");
var _getEl = function _getEl2(target) {
  return _toArray5(target)[0] || console.warn("Element not found:", target);
};
var _round11 = function _round12(value) {
  return Math.round(value * 1e4) / 1e4 || 0;
};
var _toggleClass = function _toggleClass2(targets, className, action) {
  return targets.forEach(function(el) {
    return el.classList[action](className);
  });
};
var _reserved = {
  zIndex: 1,
  kill: 1,
  simple: 1,
  spin: 1,
  clearProps: 1,
  targets: 1,
  toggleClass: 1,
  onComplete: 1,
  onUpdate: 1,
  onInterrupt: 1,
  onStart: 1,
  delay: 1,
  repeat: 1,
  repeatDelay: 1,
  yoyo: 1,
  scale: 1,
  fade: 1,
  absolute: 1,
  props: 1,
  onEnter: 1,
  onLeave: 1,
  custom: 1,
  paused: 1,
  nested: 1,
  prune: 1,
  absoluteOnLeave: 1
};
var _fitReserved = {
  zIndex: 1,
  simple: 1,
  clearProps: 1,
  scale: 1,
  absolute: 1,
  fitChild: 1,
  getVars: 1,
  props: 1
};
var _camelToDashed = function _camelToDashed2(p2) {
  return p2.replace(/([A-Z])/g, "-$1").toLowerCase();
};
var _copy = function _copy2(obj, exclude) {
  var result = {}, p2;
  for (p2 in obj) {
    exclude[p2] || (result[p2] = obj[p2]);
  }
  return result;
};
var _memoizedProps = {};
var _memoizeProps = function _memoizeProps2(props) {
  var p2 = _memoizedProps[props] = _listToArray(props);
  _memoizedRemoveProps[props] = p2.concat(_removeProps);
  return p2;
};
var _getInverseGlobalMatrix = function _getInverseGlobalMatrix2(el) {
  var cache = el._gsap || gsap12.core.getCache(el);
  if (cache.gmCache === gsap12.ticker.frame) {
    return cache.gMatrix;
  }
  cache.gmCache = gsap12.ticker.frame;
  return cache.gMatrix = getGlobalMatrix(el, true, false, true);
};
var _getDOMDepth = function _getDOMDepth2(el, invert, level) {
  if (level === void 0) {
    level = 0;
  }
  var parent = el.parentNode, inc = 1e3 * Math.pow(10, level) * (invert ? -1 : 1), l = invert ? -inc * 900 : 0;
  while (el) {
    l += inc;
    el = el.previousSibling;
  }
  return parent ? l + _getDOMDepth2(parent, invert, level + 1) : l;
};
var _orderByDOMDepth = function _orderByDOMDepth2(comps, invert, isElStates) {
  comps.forEach(function(comp) {
    return comp.d = _getDOMDepth(isElStates ? comp.element : comp.t, invert);
  });
  comps.sort(function(c1, c2) {
    return c1.d - c2.d;
  });
  return comps;
};
var _recordInlineStyles = function _recordInlineStyles2(elState, props) {
  var style = elState.element.style, a = elState.css = elState.css || [], i2 = props.length, p2, v;
  while (i2--) {
    p2 = props[i2];
    v = style[p2] || style.getPropertyValue(p2);
    a.push(v ? p2 : _dashedNameLookup[p2] || (_dashedNameLookup[p2] = _camelToDashed(p2)), v);
  }
  return style;
};
var _applyInlineStyles = function _applyInlineStyles2(state) {
  var css = state.css, style = state.element.style, i2 = 0;
  state.cache.uncache = 1;
  for (; i2 < css.length; i2 += 2) {
    css[i2 + 1] ? style[css[i2]] = css[i2 + 1] : style.removeProperty(css[i2]);
  }
  if (!css[css.indexOf("transform") + 1] && style.translate) {
    style.removeProperty("translate");
    style.removeProperty("scale");
    style.removeProperty("rotate");
  }
};
var _setFinalStates = function _setFinalStates2(comps, onlyTransforms) {
  comps.forEach(function(c) {
    return c.a.cache.uncache = 1;
  });
  onlyTransforms || comps.finalStates.forEach(_applyInlineStyles);
};
var _absoluteProps = "paddingTop,paddingRight,paddingBottom,paddingLeft,gridArea,transition".split(",");
var _makeAbsolute = function _makeAbsolute2(elState, fallbackNode, ignoreBatch) {
  var element = elState.element, width = elState.width, height = elState.height, uncache = elState.uncache, getProp = elState.getProp, style = element.style, i2 = 4, result, displayIsNone, cs;
  typeof fallbackNode !== "object" && (fallbackNode = elState);
  if (_batch && ignoreBatch !== 1) {
    _batch._abs.push({
      t: element,
      b: elState,
      a: elState,
      sd: 0
    });
    _batch._final.push(function() {
      return (elState.cache.uncache = 1) && _applyInlineStyles(elState);
    });
    return element;
  }
  displayIsNone = getProp("display") === "none";
  if (!elState.isVisible || displayIsNone) {
    displayIsNone && (_recordInlineStyles(elState, ["display"]).display = fallbackNode.display);
    elState.matrix = fallbackNode.matrix;
    elState.width = width = elState.width || fallbackNode.width;
    elState.height = height = elState.height || fallbackNode.height;
  }
  _recordInlineStyles(elState, _absoluteProps);
  cs = window.getComputedStyle(element);
  while (i2--) {
    style[_absoluteProps[i2]] = cs[_absoluteProps[i2]];
  }
  style.gridArea = "1 / 1 / 1 / 1";
  style.transition = "none";
  style.position = "absolute";
  style.width = width + "px";
  style.height = height + "px";
  style.top || (style.top = "0px");
  style.left || (style.left = "0px");
  if (uncache) {
    result = new ElementState(element);
  } else {
    result = _copy(elState, _emptyObj);
    result.position = "absolute";
    if (elState.simple) {
      var bounds = element.getBoundingClientRect();
      result.matrix = new Matrix2D(1, 0, 0, 1, bounds.left + _getDocScrollLeft(), bounds.top + _getDocScrollTop());
    } else {
      result.matrix = getGlobalMatrix(element, false, false, true);
    }
  }
  result = _fit(result, elState, true);
  elState.x = _closestTenth(result.x, 0.01);
  elState.y = _closestTenth(result.y, 0.01);
  return element;
};
var _filterComps = function _filterComps2(comps, targets) {
  if (targets !== true) {
    targets = _toArray5(targets);
    comps = comps.filter(function(c) {
      if (targets.indexOf((c.sd < 0 ? c.b : c.a).element) !== -1) {
        return true;
      } else {
        c.t._gsap.renderTransform(1);
        if (c.b.isVisible) {
          c.t.style.width = c.b.width + "px";
          c.t.style.height = c.b.height + "px";
        }
      }
    });
  }
  return comps;
};
var _makeCompsAbsolute = function _makeCompsAbsolute2(comps) {
  return _orderByDOMDepth(comps, true).forEach(function(c) {
    return (c.a.isVisible || c.b.isVisible) && _makeAbsolute(c.sd < 0 ? c.b : c.a, c.b, 1);
  });
};
var _findElStateInState = function _findElStateInState2(state, other) {
  return other && state.idLookup[_parseElementState(other).id] || state.elementStates[0];
};
var _parseElementState = function _parseElementState2(elOrNode, props, simple, other) {
  return elOrNode instanceof ElementState ? elOrNode : elOrNode instanceof FlipState ? _findElStateInState(elOrNode, other) : new ElementState(typeof elOrNode === "string" ? _getEl(elOrNode) || console.warn(elOrNode + " not found") : elOrNode, props, simple);
};
var _recordProps = function _recordProps2(elState, props) {
  var getProp = gsap12.getProperty(elState.element, null, "native"), obj = elState.props = {}, i2 = props.length;
  while (i2--) {
    obj[props[i2]] = (getProp(props[i2]) + "").trim();
  }
  obj.zIndex && (obj.zIndex = parseFloat(obj.zIndex) || 0);
  return elState;
};
var _applyProps = function _applyProps2(element, props) {
  var style = element.style || element, p2;
  for (p2 in props) {
    style[p2] = props[p2];
  }
};
var _getID = function _getID2(el) {
  var id = el.getAttribute("data-flip-id");
  id || el.setAttribute("data-flip-id", id = "auto-" + _id++);
  return id;
};
var _elementsFromElementStates = function _elementsFromElementStates2(elStates) {
  return elStates.map(function(elState) {
    return elState.element;
  });
};
var _handleCallback = function _handleCallback2(callback, elStates, tl) {
  return callback && elStates.length && tl.add(callback(_elementsFromElementStates(elStates), tl, new FlipState(elStates, 0, true)), 0);
};
var _fit = function _fit2(fromState, toState, scale, applyProps, fitChild, vars) {
  var element = fromState.element, cache = fromState.cache, parent = fromState.parent, x = fromState.x, y = fromState.y, width = toState.width, height = toState.height, scaleX = toState.scaleX, scaleY = toState.scaleY, rotation = toState.rotation, bounds = toState.bounds, styles = vars && _getStyleSaver5 && _getStyleSaver5(element, "transform,width,height"), dimensionState = fromState, _toState$matrix = toState.matrix, e = _toState$matrix.e, f = _toState$matrix.f, deep = fromState.bounds.width !== bounds.width || fromState.bounds.height !== bounds.height || fromState.scaleX !== scaleX || fromState.scaleY !== scaleY || fromState.rotation !== rotation, simple = !deep && fromState.simple && toState.simple && !fitChild, skewX, fromPoint, toPoint, getProp, parentMatrix, matrix, bbox;
  if (simple || !parent) {
    scaleX = scaleY = 1;
    rotation = skewX = 0;
  } else {
    parentMatrix = _getInverseGlobalMatrix(parent);
    matrix = parentMatrix.clone().multiply(toState.ctm ? toState.matrix.clone().multiply(toState.ctm) : toState.matrix);
    rotation = _round11(Math.atan2(matrix.b, matrix.a) * _RAD2DEG3);
    skewX = _round11(Math.atan2(matrix.c, matrix.d) * _RAD2DEG3 + rotation) % 360;
    scaleX = Math.sqrt(Math.pow(matrix.a, 2) + Math.pow(matrix.b, 2));
    scaleY = Math.sqrt(Math.pow(matrix.c, 2) + Math.pow(matrix.d, 2)) * Math.cos(skewX * _DEG2RAD4);
    if (fitChild) {
      fitChild = _toArray5(fitChild)[0];
      getProp = gsap12.getProperty(fitChild);
      bbox = fitChild.getBBox && typeof fitChild.getBBox === "function" && fitChild.getBBox();
      dimensionState = {
        scaleX: getProp("scaleX"),
        scaleY: getProp("scaleY"),
        width: bbox ? bbox.width : Math.ceil(parseFloat(getProp("width", "px"))),
        height: bbox ? bbox.height : parseFloat(getProp("height", "px"))
      };
    }
    cache.rotation = rotation + "deg";
    cache.skewX = skewX + "deg";
  }
  if (scale) {
    scaleX *= width === dimensionState.width || !dimensionState.width ? 1 : width / dimensionState.width;
    scaleY *= height === dimensionState.height || !dimensionState.height ? 1 : height / dimensionState.height;
    cache.scaleX = scaleX;
    cache.scaleY = scaleY;
  } else {
    width = _closestTenth(width * scaleX / dimensionState.scaleX, 0);
    height = _closestTenth(height * scaleY / dimensionState.scaleY, 0);
    element.style.width = width + "px";
    element.style.height = height + "px";
  }
  applyProps && _applyProps(element, toState.props);
  if (simple || !parent) {
    x += e - fromState.matrix.e;
    y += f - fromState.matrix.f;
  } else if (deep || parent !== toState.parent) {
    cache.x = x + "px";
    cache.y = y + "px";
    cache.renderTransform(1, cache);
    matrix = getGlobalMatrix(fitChild || element, false, false, true);
    fromPoint = parentMatrix.apply({
      x: matrix.e,
      y: matrix.f
    });
    toPoint = parentMatrix.apply({
      x: e,
      y: f
    });
    x += toPoint.x - fromPoint.x;
    y += toPoint.y - fromPoint.y;
  } else {
    parentMatrix.e = parentMatrix.f = 0;
    toPoint = parentMatrix.apply({
      x: e - fromState.matrix.e,
      y: f - fromState.matrix.f
    });
    x += toPoint.x;
    y += toPoint.y;
  }
  x = _closestTenth(x, 0.02);
  y = _closestTenth(y, 0.02);
  if (vars && !(vars instanceof ElementState)) {
    styles && styles.revert();
  } else {
    cache.x = x + "px";
    cache.y = y + "px";
    cache.renderTransform(1, cache);
  }
  if (vars) {
    vars.x = x;
    vars.y = y;
    vars.rotation = rotation;
    vars.skewX = skewX;
    if (scale) {
      vars.scaleX = scaleX;
      vars.scaleY = scaleY;
    } else {
      vars.width = width;
      vars.height = height;
    }
  }
  return vars || cache;
};
var _parseState = function _parseState2(targetsOrState, vars) {
  return targetsOrState instanceof FlipState ? targetsOrState : new FlipState(targetsOrState, vars);
};
var _getChangingElState = function _getChangingElState2(toState, fromState, id) {
  var to1 = toState.idLookup[id], to2 = toState.alt[id];
  return to2.isVisible && (!(fromState.getElementState(to2.element) || to2).isVisible || !to1.isVisible) ? to2 : to1;
};
var _bodyMetrics = [];
var _bodyProps = "width,height,overflowX,overflowY".split(",");
var _bodyLocked;
var _lockBodyScroll = function _lockBodyScroll2(lock) {
  if (lock !== _bodyLocked) {
    var s = _body2.style, w = _body2.clientWidth === window.outerWidth, h = _body2.clientHeight === window.outerHeight, i2 = 4;
    if (lock && (w || h)) {
      while (i2--) {
        _bodyMetrics[i2] = s[_bodyProps[i2]];
      }
      if (w) {
        s.width = _body2.clientWidth + "px";
        s.overflowY = "hidden";
      }
      if (h) {
        s.height = _body2.clientHeight + "px";
        s.overflowX = "hidden";
      }
      _bodyLocked = lock;
    } else if (_bodyLocked) {
      while (i2--) {
        _bodyMetrics[i2] ? s[_bodyProps[i2]] = _bodyMetrics[i2] : s.removeProperty(_camelToDashed(_bodyProps[i2]));
      }
      _bodyLocked = lock;
    }
  }
};
var _revertTempStyles = function _revertTempStyles2(temps, stateIndex) {
  for (var i2 = 0; i2 < temps.length; i2 += 3) {
    gsap12.set(temps[i2], {
      clearProps: true
    });
    temps[i2].setAttribute("style", temps[i2 + stateIndex]);
    temps[i2]._gsap.gmCache = -1;
  }
};
var _fromTo = function _fromTo2(fromState, toState, vars, relative) {
  fromState instanceof FlipState && toState instanceof FlipState || console.warn("Not a valid state object.");
  vars = vars || {};
  var _vars = vars, clearProps2 = _vars.clearProps, onEnter = _vars.onEnter, onLeave = _vars.onLeave, absolute = _vars.absolute, absoluteOnLeave = _vars.absoluteOnLeave, custom = _vars.custom, delay = _vars.delay, paused = _vars.paused, repeat = _vars.repeat, repeatDelay = _vars.repeatDelay, yoyo = _vars.yoyo, toggleClass = _vars.toggleClass, nested = _vars.nested, _zIndex = _vars.zIndex, scale = _vars.scale, fade = _vars.fade, stagger = _vars.stagger, spin = _vars.spin, prune = _vars.prune, props = ("props" in vars ? vars : fromState).props, tweenVars = _copy(vars, _reserved), animation = gsap12.timeline({
    delay,
    paused,
    repeat,
    repeatDelay,
    yoyo,
    data: "isFlip"
  }), remainingProps = tweenVars, entering = [], leaving = [], comps = [], swapOutTargets = [], spinNum = spin === true ? 1 : spin || 0, spinFunc = typeof spin === "function" ? spin : function() {
    return spinNum;
  }, interrupted = fromState.interrupted || toState.interrupted, addFunc = animation[relative !== 1 ? "to" : "from"], v, p2, endTime, i2, el, comp, state, targets, finalStates, fromNode, toNode, run, a, b;
  for (p2 in toState.idLookup) {
    toNode = !toState.alt[p2] ? toState.idLookup[p2] : _getChangingElState(toState, fromState, p2);
    el = toNode.element;
    fromNode = fromState.idLookup[p2];
    fromState.alt[p2] && el === fromNode.element && (fromState.alt[p2].isVisible || !toNode.isVisible) && (fromNode = fromState.alt[p2]);
    if (fromNode) {
      comp = {
        t: el,
        b: fromNode,
        a: toNode,
        sd: fromNode.element === el ? 0 : toNode.isVisible ? 1 : -1
      };
      comps.push(comp);
      if (comp.sd) {
        if (comp.sd < 0) {
          comp.b = toNode;
          comp.a = fromNode;
        }
        interrupted && _recordInlineStyles(comp.b, props ? _memoizedRemoveProps[props] : _removeProps);
        fade && comps.push(comp.swap = {
          t: fromNode.element,
          b: comp.b,
          a: comp.a,
          sd: -comp.sd,
          swap: comp
        });
      }
      el._flip = fromNode.element._flip = _batch ? _batch.timeline : animation;
    } else if (toNode.isVisible) {
      comps.push({
        t: el,
        b: _copy(toNode, {
          isVisible: 1
        }),
        a: toNode,
        sd: 0,
        entering: 1
      });
      el._flip = _batch ? _batch.timeline : animation;
    }
  }
  props && (_memoizedProps[props] || _memoizeProps(props)).forEach(function(p3) {
    return tweenVars[p3] = function(i3) {
      return comps[i3].a.props[p3];
    };
  });
  comps.finalStates = finalStates = [];
  run = function run2() {
    _orderByDOMDepth(comps);
    _lockBodyScroll(true);
    var recordedStyles = [];
    for (i2 = 0; i2 < comps.length; i2++) {
      comp = comps[i2];
      a = comp.a;
      b = comp.b;
      if (prune && !a.isDifferent(b) && !comp.entering) {
        comps.splice(i2--, 1);
      } else {
        el = comp.t;
        if (nested && !(comp.sd < 0) && i2) {
          a = comp.a = a.clone({
            matrix: getGlobalMatrix(el, false, false, true)
          });
        }
        if (b.isVisible && a.isVisible) {
          if (comp.sd < 0) {
            nested && _revertTempStyles(recordedStyles, 1);
            state = new ElementState(el, props, fromState.simple);
            _fit(state, a, scale, 0, 0, state);
            state.matrix = getGlobalMatrix(el, false, false, true);
            state.bounds = el.getBoundingClientRect();
            state.css = comp.b.css;
            comp.a = a = state;
            fade && (el.style.opacity = interrupted ? b.opacity : a.opacity);
            stagger && swapOutTargets.push(el);
            if (nested) {
              _revertTempStyles(recordedStyles, 2);
              recordedStyles.push(el, el.getAttribute("style"));
            }
          } else if (comp.sd > 0 && fade) {
            el.style.opacity = interrupted ? a.opacity - b.opacity : "0";
          }
          _fit(a, b, scale, props);
          nested && comp.sd < 0 && recordedStyles.push(el.getAttribute("style"));
        } else if (b.isVisible !== a.isVisible) {
          if (!b.isVisible) {
            a.isVisible && entering.push(a);
            comps.splice(i2--, 1);
          } else if (!a.isVisible) {
            b.css = a.css;
            leaving.push(b);
            comps.splice(i2--, 1);
            absolute && nested && _fit(a, b, scale, props);
          }
        }
        if (!scale) {
          el.style.maxWidth = Math.max(a.width, b.width) + "px";
          el.style.maxHeight = Math.max(a.height, b.height) + "px";
          el.style.minWidth = Math.min(a.width, b.width) + "px";
          el.style.minHeight = Math.min(a.height, b.height) + "px";
        }
        nested && toggleClass && el.classList.add(toggleClass);
      }
      finalStates.push(a);
    }
    var classTargets;
    if (toggleClass) {
      classTargets = finalStates.map(function(s) {
        return s.element;
      });
      nested && classTargets.forEach(function(e) {
        return e.classList.remove(toggleClass);
      });
    }
    _lockBodyScroll(false);
    if (scale) {
      tweenVars.scaleX = function(i3) {
        return comps[i3].a.scaleX;
      };
      tweenVars.scaleY = function(i3) {
        return comps[i3].a.scaleY;
      };
    } else {
      tweenVars.width = function(i3) {
        return comps[i3].a.width + "px";
      };
      tweenVars.height = function(i3) {
        return comps[i3].a.height + "px";
      };
      tweenVars.autoRound = vars.autoRound || false;
    }
    tweenVars.x = function(i3) {
      return comps[i3].a.x + "px";
    };
    tweenVars.y = function(i3) {
      return comps[i3].a.y + "px";
    };
    tweenVars.rotation = function(i3) {
      return comps[i3].a.rotation + (spin ? spinFunc(i3, targets[i3], targets) * 360 : 0);
    };
    tweenVars.skewX = function(i3) {
      return comps[i3].a.skewX;
    };
    targets = comps.map(function(c) {
      return c.t;
    });
    if (_zIndex || _zIndex === 0) {
      tweenVars.modifiers = {
        zIndex: function zIndex() {
          return _zIndex;
        }
      };
      tweenVars.zIndex = _zIndex;
      tweenVars.immediateRender = vars.immediateRender !== false;
    }
    fade && (tweenVars.opacity = function(i3) {
      return comps[i3].sd < 0 ? 0 : comps[i3].sd > 0 ? comps[i3].a.opacity : "+=0";
    });
    if (swapOutTargets.length) {
      stagger = gsap12.utils.distribute(stagger);
      var dummyArray = targets.slice(swapOutTargets.length);
      tweenVars.stagger = function(i3, el2) {
        return stagger(~swapOutTargets.indexOf(el2) ? targets.indexOf(comps[i3].swap.t) : i3, el2, dummyArray);
      };
    }
    _callbacks.forEach(function(name) {
      return vars[name] && animation.eventCallback(name, vars[name], vars[name + "Params"]);
    });
    if (custom && targets.length) {
      remainingProps = _copy(tweenVars, _reserved);
      if ("scale" in custom) {
        custom.scaleX = custom.scaleY = custom.scale;
        delete custom.scale;
      }
      for (p2 in custom) {
        v = _copy(custom[p2], _fitReserved);
        v[p2] = tweenVars[p2];
        !("duration" in v) && "duration" in tweenVars && (v.duration = tweenVars.duration);
        v.stagger = tweenVars.stagger;
        addFunc.call(animation, targets, v, 0);
        delete remainingProps[p2];
      }
    }
    if (targets.length || leaving.length || entering.length) {
      toggleClass && animation.add(function() {
        return _toggleClass(classTargets, toggleClass, animation._zTime < 0 ? "remove" : "add");
      }, 0) && !paused && _toggleClass(classTargets, toggleClass, "add");
      targets.length && addFunc.call(animation, targets, remainingProps, 0);
    }
    _handleCallback(onEnter, entering, animation);
    _handleCallback(onLeave, leaving, animation);
    var batchTl = _batch && _batch.timeline;
    if (batchTl) {
      batchTl.add(animation, 0);
      _batch._final.push(function() {
        return _setFinalStates(comps, !clearProps2);
      });
    }
    endTime = animation.duration();
    animation.call(function() {
      var forward = animation.time() >= endTime;
      forward && !batchTl && _setFinalStates(comps, !clearProps2);
      toggleClass && _toggleClass(classTargets, toggleClass, forward ? "remove" : "add");
    });
  };
  absoluteOnLeave && (absolute = comps.filter(function(comp2) {
    return !comp2.sd && !comp2.a.isVisible && comp2.b.isVisible;
  }).map(function(comp2) {
    return comp2.a.element;
  }));
  if (_batch) {
    var _batch$_abs;
    absolute && (_batch$_abs = _batch._abs).push.apply(_batch$_abs, _filterComps(comps, absolute));
    _batch._run.push(run);
  } else {
    absolute && _makeCompsAbsolute(_filterComps(comps, absolute));
    run();
  }
  var anim = _batch ? _batch.timeline : animation;
  anim.revert = function() {
    return _killFlip(anim, 1, 1);
  };
  return anim;
};
var _interrupt3 = function _interrupt4(tl) {
  tl.vars.onInterrupt && tl.vars.onInterrupt.apply(tl, tl.vars.onInterruptParams || []);
  tl.getChildren(true, false, true).forEach(_interrupt4);
};
var _killFlip = function _killFlip2(tl, action, force) {
  if (tl && tl.progress() < 1 && (!tl.paused() || force)) {
    if (action) {
      _interrupt3(tl);
      action < 2 && tl.progress(1);
      tl.kill();
    }
    return true;
  }
};
var _createLookup = function _createLookup2(state) {
  var lookup = state.idLookup = {}, alt = state.alt = {}, elStates = state.elementStates, i2 = elStates.length, elState;
  while (i2--) {
    elState = elStates[i2];
    lookup[elState.id] ? alt[elState.id] = elState : lookup[elState.id] = elState;
  }
};
var FlipState = /* @__PURE__ */ (function() {
  function FlipState2(targets, vars, targetsAreElementStates) {
    this.props = vars && vars.props;
    this.simple = !!(vars && vars.simple);
    if (targetsAreElementStates) {
      this.targets = _elementsFromElementStates(targets);
      this.elementStates = targets;
      _createLookup(this);
    } else {
      this.targets = _toArray5(targets);
      var soft = vars && (vars.kill === false || vars.batch && !vars.kill);
      _batch && !soft && _batch._kill.push(this);
      this.update(soft || !!_batch);
    }
  }
  var _proto = FlipState2.prototype;
  _proto.update = function update(soft) {
    var _this = this;
    this.elementStates = this.targets.map(function(el) {
      return new ElementState(el, _this.props, _this.simple);
    });
    _createLookup(this);
    this.interrupt(soft);
    this.recordInlineStyles();
    return this;
  };
  _proto.clear = function clear() {
    this.targets.length = this.elementStates.length = 0;
    _createLookup(this);
    return this;
  };
  _proto.fit = function fit(state, scale, nested) {
    var elStatesInOrder = _orderByDOMDepth(this.elementStates.slice(0), false, true), toElStates = (state || this).idLookup, i2 = 0, fromNode, toNode;
    for (; i2 < elStatesInOrder.length; i2++) {
      fromNode = elStatesInOrder[i2];
      nested && (fromNode.matrix = getGlobalMatrix(fromNode.element, false, false, true));
      toNode = toElStates[fromNode.id];
      toNode && _fit(fromNode, toNode, scale, true, 0, fromNode);
      fromNode.matrix = getGlobalMatrix(fromNode.element, false, false, true);
    }
    return this;
  };
  _proto.getProperty = function getProperty2(element, property) {
    var es = this.getElementState(element) || _emptyObj;
    return (property in es ? es : es.props || _emptyObj)[property];
  };
  _proto.add = function add(state) {
    var i2 = state.targets.length, lookup = this.idLookup, alt = this.alt, index, es, es2;
    while (i2--) {
      es = state.elementStates[i2];
      es2 = lookup[es.id];
      if (es2 && (es.element === es2.element || alt[es.id] && alt[es.id].element === es.element)) {
        index = this.elementStates.indexOf(es.element === es2.element ? es2 : alt[es.id]);
        this.targets.splice(index, 1, state.targets[i2]);
        this.elementStates.splice(index, 1, es);
      } else {
        this.targets.push(state.targets[i2]);
        this.elementStates.push(es);
      }
    }
    state.interrupted && (this.interrupted = true);
    state.simple || (this.simple = false);
    _createLookup(this);
    return this;
  };
  _proto.compare = function compare(state) {
    var l1 = state.idLookup, l2 = this.idLookup, unchanged = [], changed = [], enter = [], leave = [], targets = [], a1 = state.alt, a2 = this.alt, place = function place2(s12, s22, el2) {
      return (s12.isVisible !== s22.isVisible ? s12.isVisible ? enter : leave : s12.isVisible ? changed : unchanged).push(el2) && targets.push(el2);
    }, placeIfDoesNotExist = function placeIfDoesNotExist2(s12, s22, el2) {
      return targets.indexOf(el2) < 0 && place(s12, s22, el2);
    }, s1, s2, p2, el, s1Alt, s2Alt, c1, c2;
    for (p2 in l1) {
      s1Alt = a1[p2];
      s2Alt = a2[p2];
      s1 = !s1Alt ? l1[p2] : _getChangingElState(state, this, p2);
      el = s1.element;
      s2 = l2[p2];
      if (s2Alt) {
        c2 = s2.isVisible || !s2Alt.isVisible && el === s2.element ? s2 : s2Alt;
        c1 = s1Alt && !s1.isVisible && !s1Alt.isVisible && c2.element === s1Alt.element ? s1Alt : s1;
        if (c1.isVisible && c2.isVisible && c1.element !== c2.element) {
          (c1.isDifferent(c2) ? changed : unchanged).push(c1.element, c2.element);
          targets.push(c1.element, c2.element);
        } else {
          place(c1, c2, c1.element);
        }
        s1Alt && c1.element === s1Alt.element && (s1Alt = l1[p2]);
        placeIfDoesNotExist(c1.element !== s2.element && s1Alt ? s1Alt : c1, s2, s2.element);
        placeIfDoesNotExist(s1Alt && s1Alt.element === s2Alt.element ? s1Alt : c1, s2Alt, s2Alt.element);
        s1Alt && placeIfDoesNotExist(s1Alt, s2Alt.element === s1Alt.element ? s2Alt : s2, s1Alt.element);
      } else {
        !s2 ? enter.push(el) : !s2.isDifferent(s1) ? unchanged.push(el) : place(s1, s2, el);
        s1Alt && placeIfDoesNotExist(s1Alt, s2, s1Alt.element);
      }
    }
    for (p2 in l2) {
      if (!l1[p2]) {
        leave.push(l2[p2].element);
        a2[p2] && leave.push(a2[p2].element);
      }
    }
    return {
      changed,
      unchanged,
      enter,
      leave
    };
  };
  _proto.recordInlineStyles = function recordInlineStyles() {
    var props = _memoizedRemoveProps[this.props] || _removeProps, i2 = this.elementStates.length;
    while (i2--) {
      _recordInlineStyles(this.elementStates[i2], props);
    }
  };
  _proto.interrupt = function interrupt(soft) {
    var _this2 = this;
    var timelines = [];
    this.targets.forEach(function(t) {
      var tl = t._flip, foundInProgress = _killFlip(tl, soft ? 0 : 1);
      soft && foundInProgress && timelines.indexOf(tl) < 0 && tl.add(function() {
        return _this2.updateVisibility();
      });
      foundInProgress && timelines.push(tl);
    });
    !soft && timelines.length && this.updateVisibility();
    this.interrupted || (this.interrupted = !!timelines.length);
  };
  _proto.updateVisibility = function updateVisibility() {
    this.elementStates.forEach(function(es) {
      var b = es.element.getBoundingClientRect();
      es.isVisible = !!(b.width || b.height || b.top || b.left);
      es.uncache = 1;
    });
  };
  _proto.getElementState = function getElementState(element) {
    return this.elementStates[this.targets.indexOf(_getEl(element))];
  };
  _proto.makeAbsolute = function makeAbsolute() {
    return _orderByDOMDepth(this.elementStates.slice(0), true, true).map(_makeAbsolute);
  };
  return FlipState2;
})();
var ElementState = /* @__PURE__ */ (function() {
  function ElementState2(element, props, simple) {
    if (element instanceof ElementState2) {
      Object.assign(this, element, props || {});
    } else {
      this.element = element;
      this.update(props, simple);
    }
  }
  var _proto2 = ElementState2.prototype;
  _proto2.isDifferent = function isDifferent(state) {
    var b1 = this.bounds, b2 = state.bounds;
    return b1.top !== b2.top || b1.left !== b2.left || b1.width !== b2.width || b1.height !== b2.height || !this.matrix.equals(state.matrix) || this.opacity !== state.opacity || this.props && state.props && JSON.stringify(this.props) !== JSON.stringify(state.props);
  };
  _proto2.clone = function clone(overrides) {
    return new ElementState2(this, overrides);
  };
  _proto2.update = function update(props, simple) {
    var self = this, element = self.element, getProp = gsap12.getProperty(element), cache = gsap12.core.getCache(element), bounds = element.getBoundingClientRect(), bbox = element.getBBox && typeof element.getBBox === "function" && element.nodeName.toLowerCase() !== "svg" && element.getBBox(), m = simple ? new Matrix2D(1, 0, 0, 1, bounds.left + _getDocScrollLeft(), bounds.top + _getDocScrollTop()) : getGlobalMatrix(element, false, false, true);
    cache.uncache = 1;
    self.getProp = getProp;
    self.element = element;
    self.id = _getID(element);
    self.matrix = m;
    self.cache = cache;
    self.bounds = bounds;
    self.isVisible = !!(bounds.width || bounds.height || bounds.left || bounds.top);
    self.display = getProp("display");
    self.position = getProp("position");
    self.parent = element.parentNode;
    self.x = getProp("x", "px");
    self.y = getProp("y", "px");
    self.scaleX = cache.scaleX;
    self.scaleY = cache.scaleY;
    self.rotation = getProp("rotation");
    self.skewX = getProp("skewX");
    self.opacity = getProp("opacity");
    self.width = bbox ? bbox.width : _closestTenth(getProp("width", "px"), 0.04);
    self.height = bbox ? bbox.height : _closestTenth(getProp("height", "px"), 0.04);
    props && _recordProps(self, _memoizedProps[props] || _memoizeProps(props));
    self.ctm = element.getCTM && element.nodeName.toLowerCase() === "svg" && _getCTM(element).inverse();
    self.simple = simple || _round11(m.a) === 1 && !_round11(m.b) && !_round11(m.c) && _round11(m.d) === 1;
    self.uncache = 0;
  };
  return ElementState2;
})();
var FlipAction = /* @__PURE__ */ (function() {
  function FlipAction2(vars, batch) {
    this.vars = vars;
    this.batch = batch;
    this.states = [];
    this.timeline = batch.timeline;
  }
  var _proto3 = FlipAction2.prototype;
  _proto3.getStateById = function getStateById(id) {
    var i2 = this.states.length;
    while (i2--) {
      if (this.states[i2].idLookup[id]) {
        return this.states[i2];
      }
    }
  };
  _proto3.kill = function kill5() {
    this.batch.remove(this);
  };
  return FlipAction2;
})();
var FlipBatch = /* @__PURE__ */ (function() {
  function FlipBatch2(id) {
    this.id = id;
    this.actions = [];
    this._kill = [];
    this._final = [];
    this._abs = [];
    this._run = [];
    this.data = {};
    this.state = new FlipState();
    this.timeline = gsap12.timeline();
  }
  var _proto4 = FlipBatch2.prototype;
  _proto4.add = function add(config3) {
    var result = this.actions.filter(function(action) {
      return action.vars === config3;
    });
    if (result.length) {
      return result[0];
    }
    result = new FlipAction(typeof config3 === "function" ? {
      animate: config3
    } : config3, this);
    this.actions.push(result);
    return result;
  };
  _proto4.remove = function remove(action) {
    var i2 = this.actions.indexOf(action);
    i2 >= 0 && this.actions.splice(i2, 1);
    return this;
  };
  _proto4.getState = function getState(merge) {
    var _this3 = this;
    var prevBatch = _batch, prevAction = _batchAction;
    _batch = this;
    this.state.clear();
    this._kill.length = 0;
    this.actions.forEach(function(action) {
      if (action.vars.getState) {
        action.states.length = 0;
        _batchAction = action;
        action.state = action.vars.getState(action);
      }
      merge && action.states.forEach(function(s) {
        return _this3.state.add(s);
      });
    });
    _batchAction = prevAction;
    _batch = prevBatch;
    this.killConflicts();
    return this;
  };
  _proto4.animate = function animate() {
    var _this4 = this;
    var prevBatch = _batch, tl = this.timeline, i2 = this.actions.length, finalStates, endTime;
    _batch = this;
    tl.clear();
    this._abs.length = this._final.length = this._run.length = 0;
    this.actions.forEach(function(a) {
      a.vars.animate && a.vars.animate(a);
      var onEnter = a.vars.onEnter, onLeave = a.vars.onLeave, targets = a.targets, s, result;
      if (targets && targets.length && (onEnter || onLeave)) {
        s = new FlipState();
        a.states.forEach(function(state) {
          return s.add(state);
        });
        result = s.compare(Flip.getState(targets));
        result.enter.length && onEnter && onEnter(result.enter);
        result.leave.length && onLeave && onLeave(result.leave);
      }
    });
    _makeCompsAbsolute(this._abs);
    this._run.forEach(function(f) {
      return f();
    });
    endTime = tl.duration();
    finalStates = this._final.slice(0);
    tl.add(function() {
      if (endTime <= tl.time()) {
        finalStates.forEach(function(f) {
          return f();
        });
        _forEachBatch(_this4, "onComplete");
      }
    });
    _batch = prevBatch;
    while (i2--) {
      this.actions[i2].vars.once && this.actions[i2].kill();
    }
    _forEachBatch(this, "onStart");
    tl.restart();
    return this;
  };
  _proto4.loadState = function loadState(done) {
    done || (done = function done2() {
      return 0;
    });
    var queue = [];
    this.actions.forEach(function(c) {
      if (c.vars.loadState) {
        var i2, f = function f2(targets) {
          targets && (c.targets = targets);
          i2 = queue.indexOf(f2);
          if (~i2) {
            queue.splice(i2, 1);
            queue.length || done();
          }
        };
        queue.push(f);
        c.vars.loadState(f);
      }
    });
    queue.length || done();
    return this;
  };
  _proto4.setState = function setState() {
    this.actions.forEach(function(c) {
      return c.targets = c.vars.setState && c.vars.setState(c);
    });
    return this;
  };
  _proto4.killConflicts = function killConflicts(soft) {
    this.state.interrupt(soft);
    this._kill.forEach(function(state) {
      return state.interrupt(soft);
    });
    return this;
  };
  _proto4.run = function run(skipGetState, merge) {
    var _this5 = this;
    if (this !== _batch) {
      skipGetState || this.getState(merge);
      this.loadState(function() {
        if (!_this5._killed) {
          _this5.setState();
          _this5.animate();
        }
      });
    }
    return this;
  };
  _proto4.clear = function clear(stateOnly) {
    this.state.clear();
    stateOnly || (this.actions.length = 0);
  };
  _proto4.getStateById = function getStateById(id) {
    var i2 = this.actions.length, s;
    while (i2--) {
      s = this.actions[i2].getStateById(id);
      if (s) {
        return s;
      }
    }
    return this.state.idLookup[id] && this.state;
  };
  _proto4.kill = function kill5() {
    this._killed = 1;
    this.clear();
    delete _batchLookup[this.id];
  };
  return FlipBatch2;
})();
var Flip = /* @__PURE__ */ (function() {
  function Flip2() {
  }
  Flip2.getState = function getState(targets, vars) {
    var state = _parseState(targets, vars);
    _batchAction && _batchAction.states.push(state);
    vars && vars.batch && Flip2.batch(vars.batch).state.add(state);
    return state;
  };
  Flip2.from = function from(state, vars) {
    vars = vars || {};
    "clearProps" in vars || (vars.clearProps = true);
    return _fromTo(state, _parseState(vars.targets || state.targets, {
      props: vars.props || state.props,
      simple: vars.simple,
      kill: !!vars.kill
    }), vars, -1);
  };
  Flip2.to = function to(state, vars) {
    return _fromTo(state, _parseState(vars.targets || state.targets, {
      props: vars.props || state.props,
      simple: vars.simple,
      kill: !!vars.kill
    }), vars, 1);
  };
  Flip2.fromTo = function fromTo(fromState, toState, vars) {
    return _fromTo(fromState, toState, vars);
  };
  Flip2.fit = function fit(fromEl, toEl, vars) {
    var v = vars ? _copy(vars, _fitReserved) : {}, _ref = vars || v, absolute = _ref.absolute, scale = _ref.scale, getVars = _ref.getVars, props = _ref.props, runBackwards = _ref.runBackwards, onComplete = _ref.onComplete, simple = _ref.simple, fitChild = vars && vars.fitChild && _getEl(vars.fitChild), before = _parseElementState(toEl, props, simple, fromEl), after = _parseElementState(fromEl, 0, simple, before), inlineProps = props ? _memoizedRemoveProps[props] : _removeProps, ctx = gsap12.context();
    props && _applyProps(v, before.props);
    _recordInlineStyles(after, inlineProps);
    if (runBackwards) {
      "immediateRender" in v || (v.immediateRender = true);
      v.onComplete = function() {
        _applyInlineStyles(after);
        onComplete && onComplete.apply(this, arguments);
      };
    }
    absolute && _makeAbsolute(after, before);
    v = _fit(after, before, scale || fitChild, !v.duration && props, fitChild, v.duration || getVars ? v : 0);
    typeof vars === "object" && "zIndex" in vars && (v.zIndex = vars.zIndex);
    ctx && !getVars && ctx.add(function() {
      return function() {
        return _applyInlineStyles(after);
      };
    });
    return getVars ? v : v.duration ? gsap12.to(after.element, v) : null;
  };
  Flip2.makeAbsolute = function makeAbsolute(targetsOrStates, vars) {
    return (targetsOrStates instanceof FlipState ? targetsOrStates : new FlipState(targetsOrStates, vars)).makeAbsolute();
  };
  Flip2.batch = function batch(id) {
    id || (id = "default");
    return _batchLookup[id] || (_batchLookup[id] = new FlipBatch(id));
  };
  Flip2.killFlipsOf = function killFlipsOf(targets, complete) {
    (targets instanceof FlipState ? targets.targets : _toArray5(targets)).forEach(function(t) {
      return t && _killFlip(t._flip, complete !== false ? 1 : 2);
    });
  };
  Flip2.isFlipping = function isFlipping(target) {
    var f = Flip2.getByTarget(target);
    return !!f && f.isActive();
  };
  Flip2.getByTarget = function getByTarget(target) {
    return (_getEl(target) || _emptyObj)._flip;
  };
  Flip2.getElementState = function getElementState(target, props) {
    return new ElementState(_getEl(target), props);
  };
  Flip2.convertCoordinates = function convertCoordinates2(fromElement, toElement, point) {
    var m = getGlobalMatrix(toElement, true, true).multiply(getGlobalMatrix(fromElement));
    return point ? m.apply(point) : m;
  };
  Flip2.register = function register8(core) {
    _body2 = typeof document !== "undefined" && document.body;
    if (_body2) {
      gsap12 = core;
      _setDoc(_body2);
      _toArray5 = gsap12.utils.toArray;
      _getStyleSaver5 = gsap12.core.getStyleSaver;
      var snap3 = gsap12.utils.snap(0.1);
      _closestTenth = function _closestTenth2(value, add) {
        return snap3(parseFloat(value) + add);
      };
    }
  };
  return Flip2;
})();
Flip.version = "3.15.0";
typeof window !== "undefined" && window.gsap && window.gsap.registerPlugin(Flip);

// examples/figma-launch/node_modules/gsap/Physics2DPlugin.js
var gsap13;
var _coreInitted10;
var _getUnit2;
var _getStyleSaver6;
var _reverting6;
var _DEG2RAD5 = Math.PI / 180;
var _getGSAP19 = function _getGSAP20() {
  return gsap13 || typeof window !== "undefined" && (gsap13 = window.gsap) && gsap13.registerPlugin && gsap13;
};
var _round13 = function _round14(value) {
  return Math.round(value * 1e4) / 1e4;
};
var _initCore17 = function _initCore18(core) {
  gsap13 = core || _getGSAP19();
  if (!_coreInitted10) {
    _getUnit2 = gsap13.utils.getUnit;
    _getStyleSaver6 = gsap13.core.getStyleSaver;
    _reverting6 = gsap13.core.reverting || function() {
    };
    _coreInitted10 = 1;
  }
};
var PhysicsProp = function PhysicsProp2(target, p2, velocity, acceleration, stepsPerTimeUnit) {
  var cache = target._gsap, curVal = cache.get(target, p2);
  this.p = p2;
  this.set = cache.set(target, p2);
  this.s = this.val = parseFloat(curVal);
  this.u = _getUnit2(curVal) || 0;
  this.vel = velocity || 0;
  this.v = this.vel / stepsPerTimeUnit;
  if (acceleration || acceleration === 0) {
    this.acc = acceleration;
    this.a = this.acc / (stepsPerTimeUnit * stepsPerTimeUnit);
  } else {
    this.acc = this.a = 0;
  }
};
var Physics2DPlugin = {
  version: "3.15.0",
  name: "physics2D",
  register: _initCore17,
  init: function init9(target, value, tween) {
    _coreInitted10 || _initCore17();
    var data = this, angle = +value.angle || 0, velocity = +value.velocity || 0, acceleration = +value.acceleration || 0, xProp = value.xProp || "x", yProp = value.yProp || "y", aAngle = value.accelerationAngle || value.accelerationAngle === 0 ? +value.accelerationAngle : angle;
    data.styles = _getStyleSaver6 && _getStyleSaver6(target, value.xProp && value.xProp !== "x" ? value.xProp + "," + value.yProp : "transform");
    data.target = target;
    data.tween = tween;
    data.step = 0;
    data.sps = 30;
    if (value.gravity) {
      acceleration = +value.gravity;
      aAngle = 90;
    }
    angle *= _DEG2RAD5;
    aAngle *= _DEG2RAD5;
    data.fr = 1 - (+value.friction || 0);
    data._props.push(xProp, yProp);
    data.xp = new PhysicsProp(target, xProp, Math.cos(angle) * velocity, Math.cos(aAngle) * acceleration, data.sps);
    data.yp = new PhysicsProp(target, yProp, Math.sin(angle) * velocity, Math.sin(aAngle) * acceleration, data.sps);
    data.skipX = data.skipY = 0;
  },
  render: function render8(ratio, data) {
    var xp = data.xp, yp = data.yp, tween = data.tween, target = data.target, step = data.step, sps = data.sps, fr = data.fr, skipX = data.skipX, skipY = data.skipY, time = tween._from ? tween._dur - tween._time : tween._time, x, y, tt, steps, remainder, i2;
    if (tween._time || !_reverting6()) {
      if (fr === 1) {
        tt = time * time * 0.5;
        x = xp.s + xp.vel * time + xp.acc * tt;
        y = yp.s + yp.vel * time + yp.acc * tt;
      } else {
        time *= sps;
        steps = i2 = (time | 0) - step;
        if (i2 < 0) {
          xp.v = xp.vel / sps;
          yp.v = yp.vel / sps;
          xp.val = xp.s;
          yp.val = yp.s;
          data.step = 0;
          steps = i2 = time | 0;
        }
        remainder = time % 1 * fr;
        while (i2--) {
          xp.v += xp.a;
          yp.v += yp.a;
          xp.v *= fr;
          yp.v *= fr;
          xp.val += xp.v;
          yp.val += yp.v;
        }
        x = xp.val + xp.v * remainder;
        y = yp.val + yp.v * remainder;
        data.step += steps;
      }
      skipX || xp.set(target, xp.p, _round13(x) + xp.u);
      skipY || yp.set(target, yp.p, _round13(y) + yp.u);
    } else {
      data.styles.revert();
    }
  },
  kill: function kill2(property) {
    if (this.xp.p === property) {
      this.skipX = 1;
    }
    if (this.yp.p === property) {
      this.skipY = 1;
    }
  }
};
_getGSAP19() && gsap13.registerPlugin(Physics2DPlugin);

// examples/figma-launch/node_modules/gsap/PhysicsPropsPlugin.js
var gsap14;
var _coreInitted11;
var _getUnit3;
var _getStyleSaver7;
var _reverting7;
var _getGSAP21 = function _getGSAP22() {
  return gsap14 || typeof window !== "undefined" && (gsap14 = window.gsap) && gsap14.registerPlugin && gsap14;
};
var _round15 = function _round16(value) {
  return Math.round(value * 1e4) / 1e4;
};
var _initCore19 = function _initCore20(core) {
  gsap14 = core || _getGSAP21();
  if (!_coreInitted11) {
    _getUnit3 = gsap14.utils.getUnit;
    _getStyleSaver7 = gsap14.core.getStyleSaver;
    _reverting7 = gsap14.core.reverting || function() {
    };
    _coreInitted11 = 1;
  }
};
var PhysicsProp3 = function PhysicsProp4(target, p2, velocity, acceleration, friction, stepsPerTimeUnit) {
  var cache = target._gsap, curVal = cache.get(target, p2);
  this.p = p2;
  this.set = cache.set(target, p2);
  this.s = this.val = parseFloat(curVal);
  this.u = _getUnit3(curVal) || 0;
  this.vel = velocity || 0;
  this.v = this.vel / stepsPerTimeUnit;
  if (acceleration || acceleration === 0) {
    this.acc = acceleration;
    this.a = this.acc / (stepsPerTimeUnit * stepsPerTimeUnit);
  } else {
    this.acc = this.a = 0;
  }
  this.fr = 1 - (friction || 0);
};
var PhysicsPropsPlugin = {
  version: "3.15.0",
  name: "physicsProps",
  register: _initCore19,
  init: function init10(target, value, tween) {
    _coreInitted11 || _initCore19();
    var data = this, p2;
    data.styles = _getStyleSaver7 && _getStyleSaver7(target);
    data.target = target;
    data.tween = tween;
    data.step = 0;
    data.sps = 30;
    data.vProps = [];
    for (p2 in value) {
      var _value$p = value[p2], velocity = _value$p.velocity, acceleration = _value$p.acceleration, friction = _value$p.friction;
      if (velocity || acceleration) {
        data.vProps.push(new PhysicsProp3(target, p2, velocity, acceleration, friction, data.sps));
        data._props.push(p2);
        _getStyleSaver7 && data.styles.save(p2);
        friction && (data.hasFr = 1);
      }
    }
  },
  render: function render9(ratio, data) {
    var vProps = data.vProps, tween = data.tween, target = data.target, step = data.step, hasFr = data.hasFr, sps = data.sps, i2 = vProps.length, time = tween._from ? tween._dur - tween._time : tween._time, curProp, steps, remainder, j, tt;
    if (tween._time || !_reverting7()) {
      if (hasFr) {
        time *= sps;
        steps = (time | 0) - step;
        if (steps < 0) {
          while (i2--) {
            curProp = vProps[i2];
            curProp.v = curProp.vel / sps;
            curProp.val = curProp.s;
          }
          i2 = vProps.length;
          data.step = step = 0;
          steps = time | 0;
        }
        remainder = time % 1;
        while (i2--) {
          curProp = vProps[i2];
          j = steps;
          while (j--) {
            curProp.v += curProp.a;
            curProp.v *= curProp.fr;
            curProp.val += curProp.v;
          }
          curProp.set(target, curProp.p, _round15(curProp.val + curProp.v * remainder * curProp.fr) + curProp.u);
        }
        data.step += steps;
      } else {
        tt = time * time * 0.5;
        while (i2--) {
          curProp = vProps[i2];
          curProp.set(target, curProp.p, _round15(curProp.s + curProp.vel * time + curProp.acc * tt) + curProp.u);
        }
      }
    } else {
      data.styles.revert();
    }
  },
  kill: function kill3(property) {
    var vProps = this.vProps, i2 = vProps.length;
    while (i2--) {
      vProps[i2].p === property && vProps.splice(i2, 1);
    }
  }
};
_getGSAP21() && gsap14.registerPlugin(PhysicsPropsPlugin);

// examples/figma-launch/node_modules/gsap/Observer.js
function _defineProperties(target, props) {
  for (var i2 = 0; i2 < props.length; i2++) {
    var descriptor = props[i2];
    descriptor.enumerable = descriptor.enumerable || false;
    descriptor.configurable = true;
    if ("value" in descriptor) descriptor.writable = true;
    Object.defineProperty(target, descriptor.key, descriptor);
  }
}
function _createClass(Constructor, protoProps, staticProps) {
  if (protoProps) _defineProperties(Constructor.prototype, protoProps);
  if (staticProps) _defineProperties(Constructor, staticProps);
  return Constructor;
}
var gsap15;
var _coreInitted12;
var _clamp3;
var _win5;
var _doc6;
var _docEl;
var _body3;
var _isTouch;
var _pointerType;
var ScrollTrigger;
var _root;
var _normalizer;
var _eventTypes;
var _context3;
var _getGSAP23 = function _getGSAP24() {
  return gsap15 || typeof window !== "undefined" && (gsap15 = window.gsap) && gsap15.registerPlugin && gsap15;
};
var _startup = 1;
var _observers = [];
var _scrollers = [];
var _proxies = [];
var _getTime = Date.now;
var _bridge = function _bridge2(name, value) {
  return value;
};
var _integrate = function _integrate2() {
  var core = ScrollTrigger.core, data = core.bridge || {}, scrollers = core._scrollers, proxies = core._proxies;
  scrollers.push.apply(scrollers, _scrollers);
  proxies.push.apply(proxies, _proxies);
  _scrollers = scrollers;
  _proxies = proxies;
  _bridge = function _bridge3(name, value) {
    return data[name](value);
  };
};
var _getProxyProp = function _getProxyProp2(element, property) {
  return ~_proxies.indexOf(element) && _proxies[_proxies.indexOf(element) + 1][property];
};
var _isViewport = function _isViewport2(el) {
  return !!~_root.indexOf(el);
};
var _addListener = function _addListener2(element, type, func, passive, capture) {
  return element.addEventListener(type, func, {
    passive: passive !== false,
    capture: !!capture
  });
};
var _removeListener = function _removeListener2(element, type, func, capture) {
  return element.removeEventListener(type, func, !!capture);
};
var _scrollLeft = "scrollLeft";
var _scrollTop = "scrollTop";
var _onScroll = function _onScroll2() {
  return _normalizer && _normalizer.isPressed || _scrollers.cache++;
};
var _scrollCacheFunc = function _scrollCacheFunc2(f, doNotCache) {
  var cachingFunc = function cachingFunc2(value) {
    if (value || value === 0) {
      _startup && (_win5.history.scrollRestoration = "manual");
      var isNormalizing = _normalizer && _normalizer.isPressed;
      value = cachingFunc2.v = Math.round(value) || (_normalizer && _normalizer.iOS ? 1 : 0);
      f(value);
      cachingFunc2.cacheID = _scrollers.cache;
      isNormalizing && _bridge("ss", value);
    } else if (doNotCache || _scrollers.cache !== cachingFunc2.cacheID || _bridge("ref")) {
      cachingFunc2.cacheID = _scrollers.cache;
      cachingFunc2.v = f();
    }
    return cachingFunc2.v + cachingFunc2.offset;
  };
  cachingFunc.offset = 0;
  return f && cachingFunc;
};
var _horizontal = {
  s: _scrollLeft,
  p: "left",
  p2: "Left",
  os: "right",
  os2: "Right",
  d: "width",
  d2: "Width",
  a: "x",
  sc: _scrollCacheFunc(function(value) {
    return arguments.length ? _win5.scrollTo(value, _vertical.sc()) : _win5.pageXOffset || _doc6[_scrollLeft] || _docEl[_scrollLeft] || _body3[_scrollLeft] || 0;
  })
};
var _vertical = {
  s: _scrollTop,
  p: "top",
  p2: "Top",
  os: "bottom",
  os2: "Bottom",
  d: "height",
  d2: "Height",
  a: "y",
  op: _horizontal,
  sc: _scrollCacheFunc(function(value) {
    return arguments.length ? _win5.scrollTo(_horizontal.sc(), value) : _win5.pageYOffset || _doc6[_scrollTop] || _docEl[_scrollTop] || _body3[_scrollTop] || 0;
  })
};
var _getTarget = function _getTarget2(t, self) {
  return (self && self._ctx && self._ctx.selector || gsap15.utils.toArray)(t)[0] || (typeof t === "string" && gsap15.config().nullTargetWarn !== false ? console.warn("Element not found:", t) : null);
};
var _isWithin = function _isWithin2(element, list) {
  var i2 = list.length;
  while (i2--) {
    if (list[i2] === element || list[i2].contains(element)) {
      return true;
    }
  }
  return false;
};
var _getScrollFunc = function _getScrollFunc2(element, _ref) {
  var s = _ref.s, sc = _ref.sc;
  _isViewport(element) && (element = _doc6.scrollingElement || _docEl);
  var i2 = _scrollers.indexOf(element), offset = sc === _vertical.sc ? 1 : 2;
  !~i2 && (i2 = _scrollers.push(element) - 1);
  _scrollers[i2 + offset] || _addListener(element, "scroll", _onScroll);
  var prev = _scrollers[i2 + offset], func = prev || (_scrollers[i2 + offset] = _scrollCacheFunc(_getProxyProp(element, s), true) || (_isViewport(element) ? sc : _scrollCacheFunc(function(value) {
    return arguments.length ? element[s] = value : element[s];
  })));
  func.target = element;
  prev || (func.smooth = gsap15.getProperty(element, "scrollBehavior") === "smooth");
  return func;
};
var _getVelocityProp = function _getVelocityProp2(value, minTimeRefresh, useDelta) {
  var v1 = value, v2 = value, t1 = _getTime(), t2 = t1, min = minTimeRefresh || 50, dropToZeroTime = Math.max(500, min * 3), update = function update2(value2, force) {
    var t = _getTime();
    if (force || t - t1 > min) {
      v2 = v1;
      v1 = value2;
      t2 = t1;
      t1 = t;
    } else if (useDelta) {
      v1 += value2;
    } else {
      v1 = v2 + (value2 - v2) / (t - t2) * (t1 - t2);
    }
  }, reset = function reset2() {
    v2 = v1 = useDelta ? 0 : v1;
    t2 = t1 = 0;
  }, getVelocity = function getVelocity2(latestValue) {
    var tOld = t2, vOld = v2, t = _getTime();
    (latestValue || latestValue === 0) && latestValue !== v1 && update(latestValue);
    return t1 === t2 || t - t2 > dropToZeroTime ? 0 : (v1 + (useDelta ? vOld : -vOld)) / ((useDelta ? t : t1) - tOld) * 1e3;
  };
  return {
    update,
    reset,
    getVelocity
  };
};
var _getEvent = function _getEvent2(e, preventDefault) {
  preventDefault && !e._gsapAllow && e.cancelable !== false && e.preventDefault();
  return e.changedTouches ? e.changedTouches[0] : e;
};
var _getAbsoluteMax = function _getAbsoluteMax2(a) {
  var max = Math.max.apply(Math, a), min = Math.min.apply(Math, a);
  return Math.abs(max) >= Math.abs(min) ? max : min;
};
var _setScrollTrigger = function _setScrollTrigger2() {
  ScrollTrigger = gsap15.core.globals().ScrollTrigger;
  ScrollTrigger && ScrollTrigger.core && _integrate();
};
var _initCore21 = function _initCore22(core) {
  gsap15 = core || _getGSAP23();
  if (!_coreInitted12 && gsap15 && typeof document !== "undefined" && document.body) {
    _win5 = window;
    _doc6 = document;
    _docEl = _doc6.documentElement;
    _body3 = _doc6.body;
    _root = [_win5, _doc6, _docEl, _body3];
    _clamp3 = gsap15.utils.clamp;
    _context3 = gsap15.core.context || function() {
    };
    _pointerType = "onpointerenter" in _body3 ? "pointer" : "mouse";
    _isTouch = Observer.isTouch = _win5.matchMedia && _win5.matchMedia("(hover: none), (pointer: coarse)").matches ? 1 : "ontouchstart" in _win5 || navigator.maxTouchPoints > 0 || navigator.msMaxTouchPoints > 0 ? 2 : 0;
    _eventTypes = Observer.eventTypes = ("ontouchstart" in _docEl ? "touchstart,touchmove,touchcancel,touchend" : !("onpointerdown" in _docEl) ? "mousedown,mousemove,mouseup,mouseup" : "pointerdown,pointermove,pointercancel,pointerup").split(",");
    setTimeout(function() {
      return _startup = 0;
    }, 500);
    _coreInitted12 = 1;
  }
  ScrollTrigger || _setScrollTrigger();
  return _coreInitted12;
};
_horizontal.op = _vertical;
_scrollers.cache = 0;
var Observer = /* @__PURE__ */ (function() {
  function Observer2(vars) {
    this.init(vars);
  }
  var _proto = Observer2.prototype;
  _proto.init = function init16(vars) {
    _coreInitted12 || _initCore21(gsap15) || console.warn("Please gsap.registerPlugin(Observer)");
    ScrollTrigger || _setScrollTrigger();
    var tolerance = vars.tolerance, dragMinimum = vars.dragMinimum, type = vars.type, target = vars.target, lineHeight = vars.lineHeight, debounce = vars.debounce, preventDefault = vars.preventDefault, onStop = vars.onStop, onStopDelay = vars.onStopDelay, ignore = vars.ignore, wheelSpeed = vars.wheelSpeed, event = vars.event, onDragStart = vars.onDragStart, onDragEnd = vars.onDragEnd, onDrag = vars.onDrag, onPress = vars.onPress, onRelease = vars.onRelease, onRight = vars.onRight, onLeft = vars.onLeft, onUp = vars.onUp, onDown = vars.onDown, onChangeX = vars.onChangeX, onChangeY = vars.onChangeY, onChange = vars.onChange, onToggleX = vars.onToggleX, onToggleY = vars.onToggleY, onHover = vars.onHover, onHoverEnd = vars.onHoverEnd, onMove = vars.onMove, ignoreCheck = vars.ignoreCheck, isNormalizer = vars.isNormalizer, onGestureStart = vars.onGestureStart, onGestureEnd = vars.onGestureEnd, onWheel = vars.onWheel, onEnable = vars.onEnable, onDisable = vars.onDisable, onClick = vars.onClick, scrollSpeed = vars.scrollSpeed, capture = vars.capture, allowClicks = vars.allowClicks, lockAxis = vars.lockAxis, onLockAxis = vars.onLockAxis;
    this.target = target = _getTarget(target) || _docEl;
    this.vars = vars;
    ignore && (ignore = gsap15.utils.toArray(ignore));
    tolerance = tolerance || 1e-9;
    dragMinimum = dragMinimum || 0;
    wheelSpeed = wheelSpeed || 1;
    scrollSpeed = scrollSpeed || 1;
    type = type || "wheel,touch,pointer";
    debounce = debounce !== false;
    lineHeight || (lineHeight = parseFloat(_win5.getComputedStyle(_body3).lineHeight) || 22);
    var id, onStopDelayedCall, dragged, moved, wheeled, locked, axis, self = this, prevDeltaX = 0, prevDeltaY = 0, passive = vars.passive || !preventDefault && vars.passive !== false, scrollFuncX = _getScrollFunc(target, _horizontal), scrollFuncY = _getScrollFunc(target, _vertical), scrollX = scrollFuncX(), scrollY = scrollFuncY(), limitToTouch = ~type.indexOf("touch") && !~type.indexOf("pointer") && _eventTypes[0] === "pointerdown", isViewport = _isViewport(target), ownerDoc = target.ownerDocument || _doc6, deltaX = [0, 0, 0], deltaY = [0, 0, 0], onClickTime = 0, clickCapture = function clickCapture2() {
      return onClickTime = _getTime();
    }, _ignoreCheck = function _ignoreCheck2(e, isPointerOrTouch) {
      return (self.event = e) && ignore && _isWithin(e.target, ignore) || isPointerOrTouch && limitToTouch && e.pointerType !== "touch" || ignoreCheck && ignoreCheck(e, isPointerOrTouch);
    }, onStopFunc = function onStopFunc2() {
      self._vx.reset();
      self._vy.reset();
      onStopDelayedCall.pause();
      onStop && onStop(self);
    }, update = function update2() {
      var dx = self.deltaX = _getAbsoluteMax(deltaX), dy = self.deltaY = _getAbsoluteMax(deltaY), changedX = Math.abs(dx) >= tolerance, changedY = Math.abs(dy) >= tolerance;
      onChange && (changedX || changedY) && onChange(self, dx, dy, deltaX, deltaY);
      if (changedX) {
        onRight && self.deltaX > 0 && onRight(self);
        onLeft && self.deltaX < 0 && onLeft(self);
        onChangeX && onChangeX(self);
        onToggleX && self.deltaX < 0 !== prevDeltaX < 0 && onToggleX(self);
        prevDeltaX = self.deltaX;
        deltaX[0] = deltaX[1] = deltaX[2] = 0;
      }
      if (changedY) {
        onDown && self.deltaY > 0 && onDown(self);
        onUp && self.deltaY < 0 && onUp(self);
        onChangeY && onChangeY(self);
        onToggleY && self.deltaY < 0 !== prevDeltaY < 0 && onToggleY(self);
        prevDeltaY = self.deltaY;
        deltaY[0] = deltaY[1] = deltaY[2] = 0;
      }
      if (moved || dragged) {
        onMove && onMove(self);
        if (dragged) {
          onDragStart && dragged === 1 && onDragStart(self);
          onDrag && onDrag(self);
          dragged = 0;
        }
        moved = false;
      }
      locked && !(locked = false) && onLockAxis && onLockAxis(self);
      if (wheeled) {
        onWheel(self);
        wheeled = false;
      }
      id = 0;
    }, onDelta = function onDelta2(x, y, index) {
      deltaX[index] += x;
      deltaY[index] += y;
      self._vx.update(x);
      self._vy.update(y);
      debounce ? id || (id = requestAnimationFrame(update)) : update();
    }, onTouchOrPointerDelta = function onTouchOrPointerDelta2(x, y) {
      if (lockAxis && !axis) {
        self.axis = axis = Math.abs(x) > Math.abs(y) ? "x" : "y";
        locked = true;
      }
      if (axis !== "y") {
        deltaX[2] += x;
        self._vx.update(x, true);
      }
      if (axis !== "x") {
        deltaY[2] += y;
        self._vy.update(y, true);
      }
      debounce ? id || (id = requestAnimationFrame(update)) : update();
    }, _onDrag = function _onDrag2(e) {
      if (_ignoreCheck(e, 1)) {
        return;
      }
      e = _getEvent(e, preventDefault);
      var x = e.clientX, y = e.clientY, dx = x - self.x, dy = y - self.y, isDragging = self.isDragging;
      self.x = x;
      self.y = y;
      if (isDragging || (dx || dy) && (Math.abs(self.startX - x) >= dragMinimum || Math.abs(self.startY - y) >= dragMinimum)) {
        dragged || (dragged = isDragging ? 2 : 1);
        isDragging || (self.isDragging = true);
        onTouchOrPointerDelta(dx, dy);
      }
    }, _onPress3 = self.onPress = function(e) {
      if (_ignoreCheck(e, 1) || e && e.button) {
        return;
      }
      self.axis = axis = null;
      onStopDelayedCall.pause();
      self.isPressed = true;
      e = _getEvent(e);
      prevDeltaX = prevDeltaY = 0;
      self.startX = self.x = e.clientX;
      self.startY = self.y = e.clientY;
      self._vx.reset();
      self._vy.reset();
      _addListener(isNormalizer ? target : ownerDoc, _eventTypes[1], _onDrag, passive, true);
      self.deltaX = self.deltaY = 0;
      onPress && onPress(self);
    }, _onRelease3 = self.onRelease = function(e) {
      if (_ignoreCheck(e, 1)) {
        return;
      }
      _removeListener(isNormalizer ? target : ownerDoc, _eventTypes[1], _onDrag, true);
      var isTrackingDrag = !isNaN(self.y - self.startY), wasDragging = self.isDragging, isDragNotClick = wasDragging && (Math.abs(self.x - self.startX) > 3 || Math.abs(self.y - self.startY) > 3), eventData = _getEvent(e);
      if (!isDragNotClick && isTrackingDrag) {
        self._vx.reset();
        self._vy.reset();
        if (preventDefault && allowClicks) {
          gsap15.delayedCall(0.08, function() {
            if (_getTime() - onClickTime > 300 && !e.defaultPrevented) {
              if (e.target.click) {
                e.target.click();
              } else if (ownerDoc.createEvent) {
                var syntheticEvent = ownerDoc.createEvent("MouseEvents");
                syntheticEvent.initMouseEvent("click", true, true, _win5, 1, eventData.screenX, eventData.screenY, eventData.clientX, eventData.clientY, false, false, false, false, 0, null);
                e.target.dispatchEvent(syntheticEvent);
              }
            }
          });
        }
      }
      self.isDragging = self.isGesturing = self.isPressed = false;
      onStop && wasDragging && !isNormalizer && onStopDelayedCall.restart(true);
      dragged && update();
      onDragEnd && wasDragging && onDragEnd(self);
      onRelease && onRelease(self, isDragNotClick);
    }, _onGestureStart = function _onGestureStart2(e) {
      return e.touches && e.touches.length > 1 && (self.isGesturing = true) && onGestureStart(e, self.isDragging);
    }, _onGestureEnd = function _onGestureEnd2() {
      return (self.isGesturing = false) || onGestureEnd(self);
    }, onScroll = function onScroll2(e) {
      if (_ignoreCheck(e)) {
        return;
      }
      var x = scrollFuncX(), y = scrollFuncY();
      onDelta((x - scrollX) * scrollSpeed, (y - scrollY) * scrollSpeed, 1);
      scrollX = x;
      scrollY = y;
      onStop && onStopDelayedCall.restart(true);
    }, _onWheel = function _onWheel2(e) {
      if (_ignoreCheck(e)) {
        return;
      }
      e = _getEvent(e, preventDefault);
      onWheel && (wheeled = true);
      var multiplier = (e.deltaMode === 1 ? lineHeight : e.deltaMode === 2 ? _win5.innerHeight : 1) * wheelSpeed;
      onDelta(e.deltaX * multiplier, e.deltaY * multiplier, 0);
      onStop && !isNormalizer && onStopDelayedCall.restart(true);
    }, _onMove3 = function _onMove4(e) {
      if (_ignoreCheck(e)) {
        return;
      }
      var x = e.clientX, y = e.clientY, dx = x - self.x, dy = y - self.y;
      self.x = x;
      self.y = y;
      moved = true;
      onStop && onStopDelayedCall.restart(true);
      (dx || dy) && onTouchOrPointerDelta(dx, dy);
    }, _onHover = function _onHover2(e) {
      self.event = e;
      onHover(self);
    }, _onHoverEnd = function _onHoverEnd2(e) {
      self.event = e;
      onHoverEnd(self);
    }, _onClick = function _onClick2(e) {
      return _ignoreCheck(e) || _getEvent(e, preventDefault) && onClick(self);
    };
    onStopDelayedCall = self._dc = gsap15.delayedCall(onStopDelay || 0.25, onStopFunc).pause();
    self.deltaX = self.deltaY = 0;
    self._vx = _getVelocityProp(0, 50, true);
    self._vy = _getVelocityProp(0, 50, true);
    self.scrollX = scrollFuncX;
    self.scrollY = scrollFuncY;
    self.isDragging = self.isGesturing = self.isPressed = false;
    _context3(this);
    self.enable = function(e) {
      if (!self.isEnabled) {
        _addListener(isViewport ? ownerDoc : target, "scroll", _onScroll);
        type.indexOf("scroll") >= 0 && _addListener(isViewport ? ownerDoc : target, "scroll", onScroll, passive, capture);
        type.indexOf("wheel") >= 0 && _addListener(target, "wheel", _onWheel, passive, capture);
        if (type.indexOf("touch") >= 0 && _isTouch || type.indexOf("pointer") >= 0) {
          _addListener(target, _eventTypes[0], _onPress3, passive, capture);
          _addListener(ownerDoc, _eventTypes[2], _onRelease3);
          _addListener(ownerDoc, _eventTypes[3], _onRelease3);
          allowClicks && _addListener(target, "click", clickCapture, true, true);
          onClick && _addListener(target, "click", _onClick);
          onGestureStart && _addListener(ownerDoc, "gesturestart", _onGestureStart);
          onGestureEnd && _addListener(ownerDoc, "gestureend", _onGestureEnd);
          onHover && _addListener(target, _pointerType + "enter", _onHover);
          onHoverEnd && _addListener(target, _pointerType + "leave", _onHoverEnd);
          onMove && _addListener(target, _pointerType + "move", _onMove3);
        }
        self.isEnabled = true;
        self.isDragging = self.isGesturing = self.isPressed = moved = dragged = false;
        self._vx.reset();
        self._vy.reset();
        scrollX = scrollFuncX();
        scrollY = scrollFuncY();
        e && e.type && _onPress3(e);
        onEnable && onEnable(self);
      }
      return self;
    };
    self.disable = function() {
      if (self.isEnabled) {
        _observers.filter(function(o) {
          return o !== self && _isViewport(o.target);
        }).length || _removeListener(isViewport ? ownerDoc : target, "scroll", _onScroll);
        if (self.isPressed) {
          self._vx.reset();
          self._vy.reset();
          _removeListener(isNormalizer ? target : ownerDoc, _eventTypes[1], _onDrag, true);
        }
        _removeListener(isViewport ? ownerDoc : target, "scroll", onScroll, capture);
        _removeListener(target, "wheel", _onWheel, capture);
        _removeListener(target, _eventTypes[0], _onPress3, capture);
        _removeListener(ownerDoc, _eventTypes[2], _onRelease3);
        _removeListener(ownerDoc, _eventTypes[3], _onRelease3);
        _removeListener(target, "click", clickCapture, true);
        _removeListener(target, "click", _onClick);
        _removeListener(ownerDoc, "gesturestart", _onGestureStart);
        _removeListener(ownerDoc, "gestureend", _onGestureEnd);
        _removeListener(target, _pointerType + "enter", _onHover);
        _removeListener(target, _pointerType + "leave", _onHoverEnd);
        _removeListener(target, _pointerType + "move", _onMove3);
        self.isEnabled = self.isPressed = self.isDragging = false;
        onDisable && onDisable(self);
      }
    };
    self.kill = self.revert = function() {
      self.disable();
      var i2 = _observers.indexOf(self);
      i2 >= 0 && _observers.splice(i2, 1);
      _normalizer === self && (_normalizer = 0);
    };
    _observers.push(self);
    isNormalizer && _isViewport(target) && (_normalizer = self);
    self.enable(event);
  };
  _createClass(Observer2, [{
    key: "velocityX",
    get: function get() {
      return this._vx.getVelocity();
    }
  }, {
    key: "velocityY",
    get: function get() {
      return this._vy.getVelocity();
    }
  }]);
  return Observer2;
})();
Observer.version = "3.15.0";
Observer.create = function(vars) {
  return new Observer(vars);
};
Observer.register = _initCore21;
Observer.getAll = function() {
  return _observers.slice();
};
Observer.getById = function(id) {
  return _observers.filter(function(o) {
    return o.vars.id === id;
  })[0];
};
_getGSAP23() && gsap15.registerPlugin(Observer);

// examples/figma-launch/node_modules/gsap/ScrollTrigger.js
var gsap16;
var _coreInitted13;
var _win6;
var _doc7;
var _docEl2;
var _body4;
var _root2;
var _resizeDelay;
var _toArray6;
var _clamp4;
var _time2;
var _syncInterval;
var _refreshing;
var _pointerIsDown;
var _transformProp3;
var _i;
var _prevWidth;
var _prevHeight;
var _autoRefresh;
var _sort;
var _suppressOverwrites2;
var _ignoreResize;
var _normalizer2;
var _ignoreMobileResize;
var _baseScreenHeight;
var _baseScreenWidth;
var _fixIOSBug;
var _context4;
var _scrollRestoration;
var _div100vh;
var _100vh;
var _isReverted;
var _clampingMax;
var _limitCallbacks;
var _startup2 = 1;
var _getTime2 = Date.now;
var _time1 = _getTime2();
var _lastScrollTime = 0;
var _enabled = 0;
var _parseClamp = function _parseClamp2(value, type, self) {
  var clamp3 = _isString5(value) && (value.substr(0, 6) === "clamp(" || value.indexOf("max") > -1);
  self["_" + type + "Clamp"] = clamp3;
  return clamp3 ? value.substr(6, value.length - 7) : value;
};
var _keepClamp = function _keepClamp2(value, clamp3) {
  return clamp3 && (!_isString5(value) || value.substr(0, 6) !== "clamp(") ? "clamp(" + value + ")" : value;
};
var _rafBugFix = function _rafBugFix2() {
  return _enabled && requestAnimationFrame(_rafBugFix2);
};
var _pointerDownHandler = function _pointerDownHandler2() {
  return _pointerIsDown = 1;
};
var _pointerUpHandler = function _pointerUpHandler2() {
  return _pointerIsDown = 0;
};
var _passThrough3 = function _passThrough4(v) {
  return v;
};
var _round17 = function _round18(value) {
  return Math.round(value * 1e5) / 1e5 || 0;
};
var _windowExists7 = function _windowExists8() {
  return typeof window !== "undefined";
};
var _getGSAP25 = function _getGSAP26() {
  return gsap16 || _windowExists7() && (gsap16 = window.gsap) && gsap16.registerPlugin && gsap16;
};
var _isViewport3 = function _isViewport4(e) {
  return !!~_root2.indexOf(e);
};
var _getViewportDimension = function _getViewportDimension2(dimensionProperty) {
  return (dimensionProperty === "Height" ? _100vh : _win6["inner" + dimensionProperty]) || _docEl2["client" + dimensionProperty] || _body4["client" + dimensionProperty];
};
var _getBoundsFunc = function _getBoundsFunc2(element) {
  return _getProxyProp(element, "getBoundingClientRect") || (_isViewport3(element) ? function() {
    _winOffsets.width = _win6.innerWidth;
    _winOffsets.height = _100vh;
    return _winOffsets;
  } : function() {
    return _getBounds(element);
  });
};
var _getSizeFunc = function _getSizeFunc2(scroller, isViewport, _ref) {
  var d = _ref.d, d2 = _ref.d2, a = _ref.a;
  return (a = _getProxyProp(scroller, "getBoundingClientRect")) ? function() {
    return a()[d];
  } : function() {
    return (isViewport ? _getViewportDimension(d2) : scroller["client" + d2]) || 0;
  };
};
var _getOffsetsFunc = function _getOffsetsFunc2(element, isViewport) {
  return !isViewport || ~_proxies.indexOf(element) ? _getBoundsFunc(element) : function() {
    return _winOffsets;
  };
};
var _maxScroll = function _maxScroll2(element, _ref2) {
  var s = _ref2.s, d2 = _ref2.d2, d = _ref2.d, a = _ref2.a;
  return Math.max(0, (s = "scroll" + d2) && (a = _getProxyProp(element, s)) ? a() - _getBoundsFunc(element)()[d] : _isViewport3(element) ? (_docEl2[s] || _body4[s]) - _getViewportDimension(d2) : element[s] - element["offset" + d2]);
};
var _iterateAutoRefresh = function _iterateAutoRefresh2(func, events) {
  for (var i2 = 0; i2 < _autoRefresh.length; i2 += 3) {
    (!events || ~events.indexOf(_autoRefresh[i2 + 1])) && func(_autoRefresh[i2], _autoRefresh[i2 + 1], _autoRefresh[i2 + 2]);
  }
};
var _isString5 = function _isString6(value) {
  return typeof value === "string";
};
var _isFunction5 = function _isFunction6(value) {
  return typeof value === "function";
};
var _isNumber5 = function _isNumber6(value) {
  return typeof value === "number";
};
var _isObject3 = function _isObject4(value) {
  return typeof value === "object";
};
var _endAnimation = function _endAnimation2(animation, reversed, pause) {
  return animation && animation.progress(reversed ? 0 : 1) && pause && animation.pause();
};
var _callback3 = function _callback4(self, func, extraParam) {
  if (self.enabled) {
    var result = self._ctx ? self._ctx.add(function() {
      return func(self, extraParam);
    }) : func(self, extraParam);
    result && result.totalTime && (self.callbackAnimation = result);
  }
};
var _abs2 = Math.abs;
var _left = "left";
var _top = "top";
var _right = "right";
var _bottom = "bottom";
var _width = "width";
var _height = "height";
var _Right = "Right";
var _Left = "Left";
var _Top = "Top";
var _Bottom = "Bottom";
var _padding = "padding";
var _margin = "margin";
var _Width = "Width";
var _Height = "Height";
var _px = "px";
var _getComputedStyle = function _getComputedStyle2(element) {
  return _win6.getComputedStyle(element.nodeType === Node.DOCUMENT_NODE ? element.scrollingElement : element);
};
var _makePositionable = function _makePositionable2(element) {
  var position = _getComputedStyle(element).position;
  element.style.position = position === "absolute" || position === "fixed" ? position : "relative";
};
var _setDefaults3 = function _setDefaults4(obj, defaults2) {
  for (var p2 in defaults2) {
    p2 in obj || (obj[p2] = defaults2[p2]);
  }
  return obj;
};
var _getBounds = function _getBounds2(element, withoutTransforms) {
  var tween = withoutTransforms && _getComputedStyle(element)[_transformProp3] !== "matrix(1, 0, 0, 1, 0, 0)" && gsap16.to(element, {
    x: 0,
    y: 0,
    xPercent: 0,
    yPercent: 0,
    rotation: 0,
    rotationX: 0,
    rotationY: 0,
    scale: 1,
    skewX: 0,
    skewY: 0
  }).progress(1), bounds = element.getBoundingClientRect ? element.getBoundingClientRect() : element.scrollingElement.getBoundingClientRect();
  tween && tween.progress(0).kill();
  return bounds;
};
var _getSize3 = function _getSize4(element, _ref3) {
  var d2 = _ref3.d2;
  return element["offset" + d2] || element["client" + d2] || 0;
};
var _getLabelRatioArray = function _getLabelRatioArray2(timeline2) {
  var a = [], labels = timeline2.labels, duration = timeline2.duration(), p2;
  for (p2 in labels) {
    a.push(labels[p2] / duration);
  }
  return a;
};
var _getClosestLabel = function _getClosestLabel2(animation) {
  return function(value) {
    return gsap16.utils.snap(_getLabelRatioArray(animation), value);
  };
};
var _snapDirectional = function _snapDirectional2(snapIncrementOrArray) {
  var snap3 = gsap16.utils.snap(snapIncrementOrArray), a = Array.isArray(snapIncrementOrArray) && snapIncrementOrArray.slice(0).sort(function(a2, b) {
    return a2 - b;
  });
  return a ? function(value, direction, threshold) {
    if (threshold === void 0) {
      threshold = 1e-3;
    }
    var i2;
    if (!direction) {
      return snap3(value);
    }
    if (direction > 0) {
      value -= threshold;
      for (i2 = 0; i2 < a.length; i2++) {
        if (a[i2] >= value) {
          return a[i2];
        }
      }
      return a[i2 - 1];
    } else {
      i2 = a.length;
      value += threshold;
      while (i2--) {
        if (a[i2] <= value) {
          return a[i2];
        }
      }
    }
    return a[0];
  } : function(value, direction, threshold) {
    if (threshold === void 0) {
      threshold = 1e-3;
    }
    var snapped = snap3(value);
    return !direction || Math.abs(snapped - value) < threshold || snapped - value < 0 === direction < 0 ? snapped : snap3(direction < 0 ? value - snapIncrementOrArray : value + snapIncrementOrArray);
  };
};
var _getLabelAtDirection = function _getLabelAtDirection2(timeline2) {
  return function(value, st) {
    return _snapDirectional(_getLabelRatioArray(timeline2))(value, st.direction);
  };
};
var _multiListener = function _multiListener2(func, element, types, callback) {
  return types.split(",").forEach(function(type) {
    return func(element, type, callback);
  });
};
var _addListener3 = function _addListener4(element, type, func, nonPassive, capture) {
  return element.addEventListener(type, func, {
    passive: !nonPassive,
    capture: !!capture
  });
};
var _removeListener3 = function _removeListener4(element, type, func, capture) {
  return element.removeEventListener(type, func, !!capture);
};
var _wheelListener = function _wheelListener2(func, el, scrollFunc) {
  scrollFunc = scrollFunc && scrollFunc.wheelHandler;
  if (scrollFunc) {
    func(el, "wheel", scrollFunc);
    func(el, "touchmove", scrollFunc);
  }
};
var _markerDefaults = {
  startColor: "green",
  endColor: "red",
  indent: 0,
  fontSize: "16px",
  fontWeight: "normal"
};
var _defaults2 = {
  toggleActions: "play",
  anticipatePin: 0
};
var _keywords = {
  top: 0,
  left: 0,
  center: 0.5,
  bottom: 1,
  right: 1
};
var _offsetToPx = function _offsetToPx2(value, size) {
  if (_isString5(value)) {
    var eqIndex = value.indexOf("="), relative = ~eqIndex ? +(value.charAt(eqIndex - 1) + 1) * parseFloat(value.substr(eqIndex + 1)) : 0;
    if (~eqIndex) {
      value.indexOf("%") > eqIndex && (relative *= size / 100);
      value = value.substr(0, eqIndex - 1);
    }
    value = relative + (value in _keywords ? _keywords[value] * size : ~value.indexOf("%") ? parseFloat(value) * size / 100 : parseFloat(value) || 0);
  }
  return value;
};
var _createMarker = function _createMarker2(type, name, container, direction, _ref4, offset, matchWidthEl, containerAnimation) {
  var startColor = _ref4.startColor, endColor = _ref4.endColor, fontSize = _ref4.fontSize, indent = _ref4.indent, fontWeight = _ref4.fontWeight;
  var e = _doc7.createElement("div"), useFixedPosition = _isViewport3(container) || _getProxyProp(container, "pinType") === "fixed", isScroller = type.indexOf("scroller") !== -1, parent = useFixedPosition ? _body4 : container.tagName === "IFRAME" ? container.contentDocument.body : container, isStart = type.indexOf("start") !== -1, color = isStart ? startColor : endColor, css = "border-color:" + color + ";font-size:" + fontSize + ";color:" + color + ";font-weight:" + fontWeight + ";pointer-events:none;white-space:nowrap;font-family:sans-serif,Arial;z-index:1000;padding:4px 8px;border-width:0;border-style:solid;";
  css += "position:" + ((isScroller || containerAnimation) && useFixedPosition ? "fixed;" : "absolute;");
  (isScroller || containerAnimation || !useFixedPosition) && (css += (direction === _vertical ? _right : _bottom) + ":" + (offset + parseFloat(indent)) + "px;");
  matchWidthEl && (css += "box-sizing:border-box;text-align:left;width:" + matchWidthEl.offsetWidth + "px;");
  e._isStart = isStart;
  e.setAttribute("class", "gsap-marker-" + type + (name ? " marker-" + name : ""));
  e.style.cssText = css;
  e.innerText = name || name === 0 ? type + "-" + name : type;
  parent.children[0] ? parent.insertBefore(e, parent.children[0]) : parent.appendChild(e);
  e._offset = e["offset" + direction.op.d2];
  _positionMarker(e, 0, direction, isStart);
  return e;
};
var _positionMarker = function _positionMarker2(marker, start, direction, flipped) {
  var vars = {
    display: "block"
  }, side = direction[flipped ? "os2" : "p2"], oppositeSide = direction[flipped ? "p2" : "os2"];
  marker._isFlipped = flipped;
  vars[direction.a + "Percent"] = flipped ? -100 : 0;
  vars[direction.a] = flipped ? "1px" : 0;
  vars["border" + side + _Width] = 1;
  vars["border" + oppositeSide + _Width] = 0;
  vars[direction.p] = start + "px";
  gsap16.set(marker, vars);
};
var _triggers = [];
var _ids = {};
var _rafID;
var _sync = function _sync2() {
  return _getTime2() - _lastScrollTime > 34 && (_rafID || (_rafID = requestAnimationFrame(_updateAll)));
};
var _onScroll3 = function _onScroll4() {
  if (!_normalizer2 || !_normalizer2.isPressed || _normalizer2.startX > _body4.clientWidth) {
    _scrollers.cache++;
    if (_normalizer2) {
      _rafID || (_rafID = requestAnimationFrame(_updateAll));
    } else {
      _updateAll();
    }
    _lastScrollTime || _dispatch3("scrollStart");
    _lastScrollTime = _getTime2();
  }
};
var _setBaseDimensions = function _setBaseDimensions2() {
  _baseScreenWidth = _win6.innerWidth;
  _baseScreenHeight = _win6.innerHeight;
};
var _onResize = function _onResize2(force) {
  _scrollers.cache++;
  (force === true || !_refreshing && !_ignoreResize && !_doc7.fullscreenElement && !_doc7.webkitFullscreenElement && (!_ignoreMobileResize || _baseScreenWidth !== _win6.innerWidth || Math.abs(_win6.innerHeight - _baseScreenHeight) > _win6.innerHeight * 0.25)) && _resizeDelay.restart(true);
};
var _listeners2 = {};
var _emptyArray3 = [];
var _softRefresh = function _softRefresh2() {
  return _removeListener3(ScrollTrigger2, "scrollEnd", _softRefresh2) || _refreshAll(true);
};
var _dispatch3 = function _dispatch4(type) {
  return _listeners2[type] && _listeners2[type].map(function(f) {
    return f();
  }) || _emptyArray3;
};
var _savedStyles = [];
var _revertRecorded = function _revertRecorded2(media) {
  for (var i2 = 0; i2 < _savedStyles.length; i2 += 5) {
    if (!media || _savedStyles[i2 + 4] && _savedStyles[i2 + 4].query === media) {
      _savedStyles[i2].style.cssText = _savedStyles[i2 + 1];
      _savedStyles[i2].getBBox && _savedStyles[i2].setAttribute("transform", _savedStyles[i2 + 2] || "");
      _savedStyles[i2 + 3].uncache = 1;
    }
  }
};
var _recordScrollPositions = function _recordScrollPositions2() {
  return _scrollers.forEach(function(obj) {
    return _isFunction5(obj) && ++obj.cacheID && (obj.rec = obj());
  });
};
var _revertAll = function _revertAll2(kill5, media) {
  var trigger;
  for (_i = 0; _i < _triggers.length; _i++) {
    trigger = _triggers[_i];
    if (trigger && (!media || trigger._ctx === media)) {
      if (kill5) {
        trigger.kill(1);
      } else {
        trigger.revert(true, true);
      }
    }
  }
  _isReverted = true;
  media && _revertRecorded(media);
  media || _dispatch3("revert");
};
var _clearScrollMemory = function _clearScrollMemory2(scrollRestoration, force) {
  _scrollers.cache++;
  (force || !_refreshingAll) && _scrollers.forEach(function(obj) {
    return _isFunction5(obj) && obj.cacheID++ && (obj.rec = 0);
  });
  _isString5(scrollRestoration) && (_win6.history.scrollRestoration = _scrollRestoration = scrollRestoration);
};
var _refreshingAll;
var _refreshID = 0;
var _queueRefreshID;
var _queueRefreshAll = function _queueRefreshAll2() {
  if (_queueRefreshID !== _refreshID) {
    var id = _queueRefreshID = _refreshID;
    requestAnimationFrame(function() {
      return id === _refreshID && _refreshAll(true);
    });
  }
};
var _refresh100vh = function _refresh100vh2() {
  _body4.appendChild(_div100vh);
  _100vh = !_normalizer2 && _div100vh.offsetHeight || _win6.innerHeight;
  _body4.removeChild(_div100vh);
};
var _hideAllMarkers = function _hideAllMarkers2(hide) {
  return _toArray6(".gsap-marker-start, .gsap-marker-end, .gsap-marker-scroller-start, .gsap-marker-scroller-end").forEach(function(el) {
    return el.style.display = hide ? "none" : "block";
  });
};
var _refreshAll = function _refreshAll2(force, skipRevert) {
  _docEl2 = _doc7.documentElement;
  _body4 = _doc7.body;
  _root2 = [_win6, _doc7, _docEl2, _body4];
  if (_lastScrollTime && !force && !_isReverted) {
    _addListener3(ScrollTrigger2, "scrollEnd", _softRefresh);
    return;
  }
  _refresh100vh();
  _refreshingAll = ScrollTrigger2.isRefreshing = true;
  _isReverted || _recordScrollPositions();
  var refreshInits = _dispatch3("refreshInit");
  _sort && ScrollTrigger2.sort();
  skipRevert || _revertAll();
  _scrollers.forEach(function(obj) {
    if (_isFunction5(obj)) {
      obj.smooth && (obj.target.style.scrollBehavior = "auto");
      obj(0);
    }
  });
  _triggers.slice(0).forEach(function(t) {
    return t.refresh();
  });
  _isReverted = false;
  _triggers.forEach(function(t) {
    if (t._subPinOffset && t.pin) {
      var prop = t.vars.horizontal ? "offsetWidth" : "offsetHeight", original = t.pin[prop];
      t.revert(true, 1);
      t.adjustPinSpacing(t.pin[prop] - original);
      t.refresh();
    }
  });
  _clampingMax = 1;
  _hideAllMarkers(true);
  _triggers.forEach(function(t) {
    var max = _maxScroll(t.scroller, t._dir), endClamp = t.vars.end === "max" || t._endClamp && t.end > max, startClamp = t._startClamp && t.start >= max;
    (endClamp || startClamp) && t.setPositions(startClamp ? max - 1 : t.start, endClamp ? Math.max(startClamp ? max : t.start + 1, max) : t.end, true);
  });
  _hideAllMarkers(false);
  _clampingMax = 0;
  refreshInits.forEach(function(result) {
    return result && result.render && result.render(-1);
  });
  _scrollers.forEach(function(obj) {
    if (_isFunction5(obj)) {
      obj.smooth && requestAnimationFrame(function() {
        return obj.target.style.scrollBehavior = "smooth";
      });
      obj.rec && obj(obj.rec);
    }
  });
  _clearScrollMemory(_scrollRestoration, 1);
  _resizeDelay.pause();
  _refreshID++;
  _refreshingAll = 2;
  _updateAll(2);
  _triggers.forEach(function(t) {
    return _isFunction5(t.vars.onRefresh) && t.vars.onRefresh(t);
  });
  _refreshingAll = ScrollTrigger2.isRefreshing = false;
  _dispatch3("refresh");
};
var _lastScroll = 0;
var _direction = 1;
var _primary;
var _updateAll = function _updateAll2(force) {
  if (force === 2 || !_refreshingAll && !_isReverted) {
    ScrollTrigger2.isUpdating = true;
    _primary && _primary.update(0);
    var l = _triggers.length, time = _getTime2(), recordVelocity = time - _time1 >= 50, scroll = l && _triggers[0].scroll();
    _direction = _lastScroll > scroll ? -1 : 1;
    _refreshingAll || (_lastScroll = scroll);
    if (recordVelocity) {
      if (_lastScrollTime && !_pointerIsDown && time - _lastScrollTime > 200) {
        _lastScrollTime = 0;
        _dispatch3("scrollEnd");
      }
      _time2 = _time1;
      _time1 = time;
    }
    if (_direction < 0) {
      _i = l;
      while (_i-- > 0) {
        _triggers[_i] && _triggers[_i].update(0, recordVelocity);
      }
      _direction = 1;
    } else {
      for (_i = 0; _i < l; _i++) {
        _triggers[_i] && _triggers[_i].update(0, recordVelocity);
      }
    }
    ScrollTrigger2.isUpdating = false;
  }
  _rafID = 0;
};
var _propNamesToCopy = [_left, _top, _bottom, _right, _margin + _Bottom, _margin + _Right, _margin + _Top, _margin + _Left, "display", "flexShrink", "float", "zIndex", "gridColumnStart", "gridColumnEnd", "gridRowStart", "gridRowEnd", "gridArea", "justifySelf", "alignSelf", "placeSelf", "order"];
var _stateProps = _propNamesToCopy.concat([_width, _height, "boxSizing", "max" + _Width, "max" + _Height, "position", _margin, _padding, _padding + _Top, _padding + _Right, _padding + _Bottom, _padding + _Left]);
var _swapPinOut = function _swapPinOut2(pin, spacer, state) {
  _setState(state);
  var cache = pin._gsap;
  if (cache.spacerIsNative) {
    _setState(cache.spacerState);
  } else if (pin._gsap.swappedIn) {
    var parent = spacer.parentNode;
    if (parent) {
      parent.insertBefore(pin, spacer);
      parent.removeChild(spacer);
    }
  }
  pin._gsap.swappedIn = false;
};
var _swapPinIn = function _swapPinIn2(pin, spacer, cs, spacerState) {
  if (!pin._gsap.swappedIn) {
    var i2 = _propNamesToCopy.length, spacerStyle = spacer.style, pinStyle = pin.style, p2;
    while (i2--) {
      p2 = _propNamesToCopy[i2];
      spacerStyle[p2] = cs[p2];
    }
    spacerStyle.position = cs.position === "absolute" ? "absolute" : "relative";
    cs.display === "inline" && (spacerStyle.display = "inline-block");
    pinStyle[_bottom] = pinStyle[_right] = "auto";
    spacerStyle.flexBasis = cs.flexBasis || "auto";
    spacerStyle.overflow = "visible";
    spacerStyle.boxSizing = "border-box";
    spacerStyle[_width] = _getSize3(pin, _horizontal) + _px;
    spacerStyle[_height] = _getSize3(pin, _vertical) + _px;
    spacerStyle[_padding] = pinStyle[_margin] = pinStyle[_top] = pinStyle[_left] = "0";
    _setState(spacerState);
    pinStyle[_width] = pinStyle["max" + _Width] = cs[_width];
    pinStyle[_height] = pinStyle["max" + _Height] = cs[_height];
    pinStyle[_padding] = cs[_padding];
    if (pin.parentNode !== spacer) {
      pin.parentNode.insertBefore(spacer, pin);
      spacer.appendChild(pin);
    }
    pin._gsap.swappedIn = true;
  }
};
var _capsExp2 = /([A-Z])/g;
var _setState = function _setState2(state) {
  if (state) {
    var style = state.t.style, l = state.length, i2 = 0, p2, value;
    (state.t._gsap || gsap16.core.getCache(state.t)).uncache = 1;
    for (; i2 < l; i2 += 2) {
      value = state[i2 + 1];
      p2 = state[i2];
      if (value) {
        style[p2] = value;
      } else if (style[p2]) {
        style.removeProperty(p2.replace(_capsExp2, "-$1").toLowerCase());
      }
    }
  }
};
var _getState = function _getState2(element) {
  var l = _stateProps.length, style = element.style, state = [], i2 = 0;
  for (; i2 < l; i2++) {
    state.push(_stateProps[i2], style[_stateProps[i2]]);
  }
  state.t = element;
  return state;
};
var _copyState = function _copyState2(state, override, omitOffsets) {
  var result = [], l = state.length, i2 = omitOffsets ? 8 : 0, p2;
  for (; i2 < l; i2 += 2) {
    p2 = state[i2];
    result.push(p2, p2 in override ? override[p2] : state[i2 + 1]);
  }
  result.t = state.t;
  return result;
};
var _winOffsets = {
  left: 0,
  top: 0
};
var _parsePosition3 = function _parsePosition4(value, trigger, scrollerSize, direction, scroll, marker, markerScroller, self, scrollerBounds, borderWidth, useFixedPosition, scrollerMax, containerAnimation, clampZeroProp) {
  _isFunction5(value) && (value = value(self));
  if (_isString5(value) && value.substr(0, 3) === "max") {
    value = scrollerMax + (value.charAt(4) === "=" ? _offsetToPx("0" + value.substr(3), scrollerSize) : 0);
  }
  var time = containerAnimation ? containerAnimation.time() : 0, p1, p2, element;
  containerAnimation && containerAnimation.seek(0);
  isNaN(value) || (value = +value);
  if (!_isNumber5(value)) {
    _isFunction5(trigger) && (trigger = trigger(self));
    var offsets = (value || "0").split(" "), bounds, localOffset, globalOffset, display;
    element = _getTarget(trigger, self) || _body4;
    bounds = _getBounds(element) || {};
    if ((!bounds || !bounds.left && !bounds.top) && _getComputedStyle(element).display === "none") {
      display = element.style.display;
      element.style.display = "block";
      bounds = _getBounds(element);
      display ? element.style.display = display : element.style.removeProperty("display");
    }
    localOffset = _offsetToPx(offsets[0], bounds[direction.d]);
    globalOffset = _offsetToPx(offsets[1] || "0", scrollerSize);
    value = bounds[direction.p] - scrollerBounds[direction.p] - borderWidth + localOffset + scroll - globalOffset;
    markerScroller && _positionMarker(markerScroller, globalOffset, direction, scrollerSize - globalOffset < 20 || markerScroller._isStart && globalOffset > 20);
    scrollerSize -= scrollerSize - globalOffset;
  } else {
    containerAnimation && (value = gsap16.utils.mapRange(containerAnimation.scrollTrigger.start, containerAnimation.scrollTrigger.end, 0, scrollerMax, value));
    markerScroller && _positionMarker(markerScroller, scrollerSize, direction, true);
  }
  if (clampZeroProp) {
    self[clampZeroProp] = value || -1e-3;
    value < 0 && (value = 0);
  }
  if (marker) {
    var position = value + scrollerSize, isStart = marker._isStart;
    p1 = "scroll" + direction.d2;
    _positionMarker(marker, position, direction, isStart && position > 20 || !isStart && (useFixedPosition ? Math.max(_body4[p1], _docEl2[p1]) : marker.parentNode[p1]) <= position + 1);
    if (useFixedPosition) {
      scrollerBounds = _getBounds(markerScroller);
      useFixedPosition && (marker.style[direction.op.p] = scrollerBounds[direction.op.p] - direction.op.m - marker._offset + _px);
    }
  }
  if (containerAnimation && element) {
    p1 = _getBounds(element);
    containerAnimation.seek(scrollerMax);
    p2 = _getBounds(element);
    containerAnimation._caScrollDist = p1[direction.p] - p2[direction.p];
    value = value / containerAnimation._caScrollDist * scrollerMax;
  }
  containerAnimation && containerAnimation.seek(time);
  return containerAnimation ? value : Math.round(value);
};
var _prefixExp = /(webkit|moz|length|cssText|inset)/i;
var _reparent = function _reparent2(element, parent, top, left) {
  if (element.parentNode !== parent) {
    var style = element.style, p2, cs;
    if (parent === _body4) {
      element._stOrig = style.cssText;
      cs = _getComputedStyle(element);
      for (p2 in cs) {
        if (!+p2 && !_prefixExp.test(p2) && cs[p2] && typeof style[p2] === "string" && p2 !== "0") {
          style[p2] = cs[p2];
        }
      }
      style.top = top;
      style.left = left;
    } else {
      style.cssText = element._stOrig;
    }
    gsap16.core.getCache(element).uncache = 1;
    parent.appendChild(element);
  }
};
var _interruptionTracker = function _interruptionTracker2(getValueFunc, initialValue, onInterrupt) {
  var last1 = initialValue, last2 = last1;
  return function(value) {
    var current = Math.round(getValueFunc());
    if (current !== last1 && current !== last2 && Math.abs(current - last1) > 3 && Math.abs(current - last2) > 3) {
      value = current;
      onInterrupt && onInterrupt();
    }
    last2 = last1;
    last1 = Math.round(value);
    return last1;
  };
};
var _shiftMarker = function _shiftMarker2(marker, direction, value) {
  var vars = {};
  vars[direction.p] = "+=" + value;
  gsap16.set(marker, vars);
};
var _getTweenCreator = function _getTweenCreator2(scroller, direction) {
  var getScroll = _getScrollFunc(scroller, direction), prop = "_scroll" + direction.p2, getTween = function getTween2(scrollTo, vars, initialValue, change1, change2) {
    var tween = getTween2.tween, onComplete = vars.onComplete, modifiers = {};
    initialValue = initialValue || getScroll();
    var checkForInterruption = _interruptionTracker(getScroll, initialValue, function() {
      tween.kill();
      getTween2.tween = 0;
    });
    change2 = change1 && change2 || 0;
    change1 = change1 || scrollTo - initialValue;
    tween && tween.kill();
    vars[prop] = scrollTo;
    vars.inherit = false;
    vars.modifiers = modifiers;
    modifiers[prop] = function() {
      return checkForInterruption(initialValue + change1 * tween.ratio + change2 * tween.ratio * tween.ratio);
    };
    vars.onUpdate = function() {
      _scrollers.cache++;
      getTween2.tween && _updateAll();
    };
    vars.onComplete = function() {
      getTween2.tween = 0;
      onComplete && onComplete.call(tween);
    };
    tween = getTween2.tween = gsap16.to(scroller, vars);
    return tween;
  };
  scroller[prop] = getScroll;
  getScroll.wheelHandler = function() {
    return getTween.tween && getTween.tween.kill() && (getTween.tween = 0);
  };
  _addListener3(scroller, "wheel", getScroll.wheelHandler);
  ScrollTrigger2.isTouch && _addListener3(scroller, "touchmove", getScroll.wheelHandler);
  return getTween;
};
var ScrollTrigger2 = /* @__PURE__ */ (function() {
  function ScrollTrigger5(vars, animation) {
    _coreInitted13 || ScrollTrigger5.register(gsap16) || console.warn("Please gsap.registerPlugin(ScrollTrigger)");
    _context4(this);
    this.init(vars, animation);
  }
  var _proto = ScrollTrigger5.prototype;
  _proto.init = function init16(vars, animation) {
    this.progress = this.start = 0;
    this.vars && this.kill(true, true);
    if (!_enabled) {
      this.update = this.refresh = this.kill = _passThrough3;
      return;
    }
    vars = _setDefaults3(_isString5(vars) || _isNumber5(vars) || vars.nodeType ? {
      trigger: vars
    } : vars, _defaults2);
    var _vars = vars, onUpdate = _vars.onUpdate, toggleClass = _vars.toggleClass, id = _vars.id, onToggle = _vars.onToggle, onRefresh = _vars.onRefresh, scrub = _vars.scrub, trigger = _vars.trigger, pin = _vars.pin, pinSpacing = _vars.pinSpacing, invalidateOnRefresh = _vars.invalidateOnRefresh, anticipatePin = _vars.anticipatePin, onScrubComplete = _vars.onScrubComplete, onSnapComplete = _vars.onSnapComplete, once = _vars.once, snap3 = _vars.snap, pinReparent = _vars.pinReparent, pinSpacer = _vars.pinSpacer, containerAnimation = _vars.containerAnimation, fastScrollEnd = _vars.fastScrollEnd, preventOverlaps = _vars.preventOverlaps, direction = vars.horizontal || vars.containerAnimation && vars.horizontal !== false ? _horizontal : _vertical, isToggle = !scrub && scrub !== 0, scroller = _getTarget(vars.scroller || _win6), scrollerCache = gsap16.core.getCache(scroller), isViewport = _isViewport3(scroller), useFixedPosition = ("pinType" in vars ? vars.pinType : _getProxyProp(scroller, "pinType") || isViewport && "fixed") === "fixed", callbacks = [vars.onEnter, vars.onLeave, vars.onEnterBack, vars.onLeaveBack], toggleActions = isToggle && vars.toggleActions.split(" "), markers = "markers" in vars ? vars.markers : _defaults2.markers, borderWidth = isViewport ? 0 : parseFloat(_getComputedStyle(scroller)["border" + direction.p2 + _Width]) || 0, self = this, onRefreshInit = vars.onRefreshInit && function() {
      return vars.onRefreshInit(self);
    }, getScrollerSize = _getSizeFunc(scroller, isViewport, direction), getScrollerOffsets = _getOffsetsFunc(scroller, isViewport), lastSnap = 0, lastRefresh = 0, prevProgress = 0, scrollFunc = _getScrollFunc(scroller, direction), tweenTo, pinCache, snapFunc, scroll1, scroll2, start, end, markerStart, markerEnd, markerStartTrigger, markerEndTrigger, markerVars, executingOnRefresh, change, pinOriginalState, pinActiveState, pinState, spacer, offset, pinGetter, pinSetter, pinStart, pinChange, spacingStart, spacerState, markerStartSetter, pinMoves, markerEndSetter, cs, snap1, snap22, scrubTween, scrubSmooth, snapDurClamp, snapDelayedCall, prevScroll, prevAnimProgress, caMarkerSetter, customRevertReturn;
    self._startClamp = self._endClamp = false;
    self._dir = direction;
    anticipatePin *= 45;
    self.scroller = scroller;
    self.scroll = containerAnimation ? containerAnimation.time.bind(containerAnimation) : scrollFunc;
    scroll1 = scrollFunc();
    self.vars = vars;
    animation = animation || vars.animation;
    if ("refreshPriority" in vars) {
      _sort = 1;
      vars.refreshPriority === -9999 && (_primary = self);
    }
    scrollerCache.tweenScroll = scrollerCache.tweenScroll || {
      top: _getTweenCreator(scroller, _vertical),
      left: _getTweenCreator(scroller, _horizontal)
    };
    self.tweenTo = tweenTo = scrollerCache.tweenScroll[direction.p];
    self.scrubDuration = function(value) {
      scrubSmooth = _isNumber5(value) && value;
      if (!scrubSmooth) {
        scrubTween && scrubTween.progress(1).kill();
        scrubTween = 0;
      } else {
        scrubTween ? scrubTween.duration(value) : scrubTween = gsap16.to(animation, {
          ease: "expo",
          totalProgress: "+=0",
          inherit: false,
          duration: scrubSmooth,
          paused: true,
          onComplete: function onComplete() {
            return onScrubComplete && onScrubComplete(self);
          }
        });
      }
    };
    if (animation) {
      animation.vars.lazy = false;
      animation._initted && !self.isReverted || animation.vars.immediateRender !== false && vars.immediateRender !== false && animation.duration() && animation.render(0, true, true);
      self.animation = animation.pause();
      animation.scrollTrigger = self;
      self.scrubDuration(scrub);
      snap1 = 0;
      id || (id = animation.vars.id);
    }
    if (snap3) {
      if (!_isObject3(snap3) || snap3.push) {
        snap3 = {
          snapTo: snap3
        };
      }
      "scrollBehavior" in _body4.style && gsap16.set(isViewport ? [_body4, _docEl2] : scroller, {
        scrollBehavior: "auto"
      });
      _scrollers.forEach(function(o) {
        return _isFunction5(o) && o.target === (isViewport ? _doc7.scrollingElement || _docEl2 : scroller) && (o.smooth = false);
      });
      snapFunc = _isFunction5(snap3.snapTo) ? snap3.snapTo : snap3.snapTo === "labels" ? _getClosestLabel(animation) : snap3.snapTo === "labelsDirectional" ? _getLabelAtDirection(animation) : snap3.directional !== false ? function(value, st) {
        return _snapDirectional(snap3.snapTo)(value, _getTime2() - lastRefresh < 500 ? 0 : st.direction);
      } : gsap16.utils.snap(snap3.snapTo);
      snapDurClamp = snap3.duration || {
        min: 0.1,
        max: 2
      };
      snapDurClamp = _isObject3(snapDurClamp) ? _clamp4(snapDurClamp.min, snapDurClamp.max) : _clamp4(snapDurClamp, snapDurClamp);
      snapDelayedCall = gsap16.delayedCall(snap3.delay || scrubSmooth / 2 || 0.1, function() {
        var scroll = scrollFunc(), refreshedRecently = _getTime2() - lastRefresh < 500, tween = tweenTo.tween;
        if ((refreshedRecently || Math.abs(self.getVelocity()) < 10) && !tween && !_pointerIsDown && lastSnap !== scroll) {
          var progress = (scroll - start) / change, totalProgress = animation && !isToggle ? animation.totalProgress() : progress, velocity = refreshedRecently ? 0 : (totalProgress - snap22) / (_getTime2() - _time2) * 1e3 || 0, change1 = gsap16.utils.clamp(-progress, 1 - progress, _abs2(velocity / 2) * velocity / 0.185), naturalEnd = progress + (snap3.inertia === false ? 0 : change1), endValue, endScroll, _snap = snap3, onStart = _snap.onStart, _onInterrupt = _snap.onInterrupt, _onComplete = _snap.onComplete;
          endValue = snapFunc(naturalEnd, self);
          _isNumber5(endValue) || (endValue = naturalEnd);
          endScroll = Math.max(0, Math.round(start + endValue * change));
          if (scroll <= end && scroll >= start && endScroll !== scroll) {
            if (tween && !tween._initted && tween.data <= _abs2(endScroll - scroll)) {
              return;
            }
            if (snap3.inertia === false) {
              change1 = endValue - progress;
            }
            tweenTo(endScroll, {
              duration: snapDurClamp(_abs2(Math.max(_abs2(naturalEnd - totalProgress), _abs2(endValue - totalProgress)) * 0.185 / velocity / 0.05 || 0)),
              ease: snap3.ease || "power3",
              data: _abs2(endScroll - scroll),
              // record the distance so that if another snap tween occurs (conflict) we can prioritize the closest snap.
              onInterrupt: function onInterrupt() {
                return snapDelayedCall.restart(true) && _onInterrupt && _callback3(self, _onInterrupt);
              },
              onComplete: function onComplete() {
                self.update();
                lastSnap = scrollFunc();
                if (animation && !isToggle) {
                  scrubTween ? scrubTween.resetTo("totalProgress", endValue, animation._tTime / animation._tDur) : animation.progress(endValue);
                }
                snap1 = snap22 = animation && !isToggle ? animation.totalProgress() : self.progress;
                onSnapComplete && onSnapComplete(self);
                _onComplete && _callback3(self, _onComplete);
              }
            }, scroll, change1 * change, endScroll - scroll - change1 * change);
            onStart && _callback3(self, onStart, tweenTo.tween);
          }
        } else if (self.isActive && lastSnap !== scroll) {
          snapDelayedCall.restart(true);
        }
      }).pause();
    }
    id && (_ids[id] = self);
    trigger = self.trigger = _getTarget(trigger || pin !== true && pin);
    customRevertReturn = trigger && trigger._gsap && trigger._gsap.stRevert;
    customRevertReturn && (customRevertReturn = customRevertReturn(self));
    pin = pin === true ? trigger : _getTarget(pin);
    _isString5(toggleClass) && (toggleClass = {
      targets: trigger,
      className: toggleClass
    });
    if (pin) {
      pinSpacing === false || pinSpacing === _margin || (pinSpacing = !pinSpacing && pin.parentNode && pin.parentNode.style && _getComputedStyle(pin.parentNode).display === "flex" ? false : _padding);
      self.pin = pin;
      pinCache = gsap16.core.getCache(pin);
      if (!pinCache.spacer) {
        if (pinSpacer) {
          pinSpacer = _getTarget(pinSpacer);
          pinSpacer && !pinSpacer.nodeType && (pinSpacer = pinSpacer.current || pinSpacer.nativeElement);
          pinCache.spacerIsNative = !!pinSpacer;
          pinSpacer && (pinCache.spacerState = _getState(pinSpacer));
        }
        pinCache.spacer = spacer = pinSpacer || _doc7.createElement("div");
        spacer.classList.add("pin-spacer");
        id && spacer.classList.add("pin-spacer-" + id);
        pinCache.pinState = pinOriginalState = _getState(pin);
      } else {
        pinOriginalState = pinCache.pinState;
      }
      vars.force3D !== false && gsap16.set(pin, {
        force3D: true
      });
      self.spacer = spacer = pinCache.spacer;
      cs = _getComputedStyle(pin);
      spacingStart = cs[pinSpacing + direction.os2];
      pinGetter = gsap16.getProperty(pin);
      pinSetter = gsap16.quickSetter(pin, direction.a, _px);
      _swapPinIn(pin, spacer, cs);
      pinState = _getState(pin);
    }
    if (markers) {
      markerVars = _isObject3(markers) ? _setDefaults3(markers, _markerDefaults) : _markerDefaults;
      markerStartTrigger = _createMarker("scroller-start", id, scroller, direction, markerVars, 0);
      markerEndTrigger = _createMarker("scroller-end", id, scroller, direction, markerVars, 0, markerStartTrigger);
      offset = markerStartTrigger["offset" + direction.op.d2];
      var content = _getTarget(_getProxyProp(scroller, "content") || scroller);
      markerStart = this.markerStart = _createMarker("start", id, content, direction, markerVars, offset, 0, containerAnimation);
      markerEnd = this.markerEnd = _createMarker("end", id, content, direction, markerVars, offset, 0, containerAnimation);
      containerAnimation && (caMarkerSetter = gsap16.quickSetter([markerStart, markerEnd], direction.a, _px));
      if (!useFixedPosition && !(_proxies.length && _getProxyProp(scroller, "fixedMarkers") === true)) {
        _makePositionable(isViewport ? _body4 : scroller);
        gsap16.set([markerStartTrigger, markerEndTrigger], {
          force3D: true
        });
        markerStartSetter = gsap16.quickSetter(markerStartTrigger, direction.a, _px);
        markerEndSetter = gsap16.quickSetter(markerEndTrigger, direction.a, _px);
      }
    }
    if (containerAnimation) {
      var oldOnUpdate = containerAnimation.vars.onUpdate, oldParams = containerAnimation.vars.onUpdateParams;
      containerAnimation.eventCallback("onUpdate", function() {
        self.update(0, 0, 1);
        oldOnUpdate && oldOnUpdate.apply(containerAnimation, oldParams || []);
      });
    }
    self.previous = function() {
      return _triggers[_triggers.indexOf(self) - 1];
    };
    self.next = function() {
      return _triggers[_triggers.indexOf(self) + 1];
    };
    self.revert = function(revert, temp) {
      if (!temp) {
        return self.kill(true);
      }
      var r = revert !== false || !self.enabled, prevRefreshing = _refreshing;
      if (r !== self.isReverted) {
        if (r) {
          prevScroll = Math.max(scrollFunc(), self.scroll.rec || 0);
          prevProgress = self.progress;
          prevAnimProgress = animation && animation.progress();
        }
        markerStart && [markerStart, markerEnd, markerStartTrigger, markerEndTrigger].forEach(function(m) {
          return m.style.display = r ? "none" : "block";
        });
        if (r) {
          _refreshing = self;
          self.update(r);
        }
        if (pin && (!pinReparent || !self.isActive)) {
          if (r) {
            _swapPinOut(pin, spacer, pinOriginalState);
          } else {
            _swapPinIn(pin, spacer, _getComputedStyle(pin), spacerState);
          }
        }
        r || self.update(r);
        _refreshing = prevRefreshing;
        self.isReverted = r;
      }
    };
    self.refresh = function(soft, force, position, pinOffset) {
      if ((_refreshing || !self.enabled) && !force) {
        return;
      }
      if (pin && soft && _lastScrollTime) {
        _addListener3(ScrollTrigger5, "scrollEnd", _softRefresh);
        return;
      }
      !_refreshingAll && onRefreshInit && onRefreshInit(self);
      _refreshing = self;
      if (tweenTo.tween && !position) {
        tweenTo.tween.kill();
        tweenTo.tween = 0;
      }
      scrubTween && scrubTween.pause();
      if (invalidateOnRefresh && animation) {
        animation.revert({
          kill: false
        }).invalidate();
        animation.getChildren ? animation.getChildren(true, true, false).forEach(function(t) {
          return t.vars.immediateRender && t.render(0, true, true);
        }) : animation.vars.immediateRender && animation.render(0, true, true);
      }
      self.isReverted || self.revert(true, true);
      self._subPinOffset = false;
      var size = getScrollerSize(), scrollerBounds = getScrollerOffsets(), max = containerAnimation ? containerAnimation.duration() : _maxScroll(scroller, direction), isFirstRefresh = change <= 0.01 || !change, offset2 = 0, otherPinOffset = pinOffset || 0, parsedEnd = _isObject3(position) ? position.end : vars.end, parsedEndTrigger = vars.endTrigger || trigger, parsedStart = _isObject3(position) ? position.start : vars.start || (vars.start === 0 || !trigger ? 0 : pin ? "0 0" : "0 100%"), pinnedContainer = self.pinnedContainer = vars.pinnedContainer && _getTarget(vars.pinnedContainer, self), triggerIndex = trigger && Math.max(0, _triggers.indexOf(self)) || 0, i2 = triggerIndex, cs2, bounds, scroll, isVertical, override, curTrigger, curPin, oppositeScroll, initted, revertedPins, forcedOverflow, markerStartOffset, markerEndOffset;
      if (markers && _isObject3(position)) {
        markerStartOffset = gsap16.getProperty(markerStartTrigger, direction.p);
        markerEndOffset = gsap16.getProperty(markerEndTrigger, direction.p);
      }
      while (i2-- > 0) {
        curTrigger = _triggers[i2];
        curTrigger.end || curTrigger.refresh(0, 1) || (_refreshing = self);
        curPin = curTrigger.pin;
        if (curPin && (curPin === trigger || curPin === pin || curPin === pinnedContainer) && !curTrigger.isReverted) {
          revertedPins || (revertedPins = []);
          revertedPins.unshift(curTrigger);
          curTrigger.revert(true, true);
        }
        if (curTrigger !== _triggers[i2]) {
          triggerIndex--;
          i2--;
        }
      }
      _isFunction5(parsedStart) && (parsedStart = parsedStart(self));
      parsedStart = _parseClamp(parsedStart, "start", self);
      start = _parsePosition3(parsedStart, trigger, size, direction, scrollFunc(), markerStart, markerStartTrigger, self, scrollerBounds, borderWidth, useFixedPosition, max, containerAnimation, self._startClamp && "_startClamp") || (pin ? -1e-3 : 0);
      _isFunction5(parsedEnd) && (parsedEnd = parsedEnd(self));
      if (_isString5(parsedEnd) && !parsedEnd.indexOf("+=")) {
        if (~parsedEnd.indexOf(" ")) {
          parsedEnd = (_isString5(parsedStart) ? parsedStart.split(" ")[0] : "") + parsedEnd;
        } else {
          offset2 = _offsetToPx(parsedEnd.substr(2), size);
          parsedEnd = _isString5(parsedStart) ? parsedStart : (containerAnimation ? gsap16.utils.mapRange(0, containerAnimation.duration(), containerAnimation.scrollTrigger.start, containerAnimation.scrollTrigger.end, start) : start) + offset2;
          parsedEndTrigger = trigger;
        }
      }
      parsedEnd = _parseClamp(parsedEnd, "end", self);
      end = Math.max(start, _parsePosition3(parsedEnd || (parsedEndTrigger ? "100% 0" : max), parsedEndTrigger, size, direction, scrollFunc() + offset2, markerEnd, markerEndTrigger, self, scrollerBounds, borderWidth, useFixedPosition, max, containerAnimation, self._endClamp && "_endClamp")) || -1e-3;
      offset2 = 0;
      i2 = triggerIndex;
      while (i2--) {
        curTrigger = _triggers[i2] || {};
        curPin = curTrigger.pin;
        if (curPin && curTrigger.start - curTrigger._pinPush <= start && !containerAnimation && curTrigger.end > 0) {
          cs2 = curTrigger.end - (self._startClamp ? Math.max(0, curTrigger.start) : curTrigger.start);
          if ((curPin === trigger && curTrigger.start - curTrigger._pinPush < start || curPin === pinnedContainer) && isNaN(parsedStart)) {
            offset2 += cs2 * (1 - curTrigger.progress);
          }
          curPin === pin && (otherPinOffset += cs2);
        }
      }
      start += offset2;
      end += offset2;
      self._startClamp && (self._startClamp += offset2);
      if (self._endClamp && !_refreshingAll) {
        self._endClamp = end || -1e-3;
        end = Math.min(end, _maxScroll(scroller, direction));
      }
      change = end - start || (start -= 0.01) && 1e-3;
      if (isFirstRefresh) {
        prevProgress = gsap16.utils.clamp(0, 1, gsap16.utils.normalize(start, end, prevScroll));
      }
      self._pinPush = otherPinOffset;
      if (markerStart && offset2) {
        cs2 = {};
        cs2[direction.a] = "+=" + offset2;
        pinnedContainer && (cs2[direction.p] = "-=" + scrollFunc());
        gsap16.set([markerStart, markerEnd], cs2);
      }
      if (pin && !(_clampingMax && self.end >= _maxScroll(scroller, direction))) {
        cs2 = _getComputedStyle(pin);
        isVertical = direction === _vertical;
        scroll = scrollFunc();
        pinStart = parseFloat(pinGetter(direction.a)) + otherPinOffset;
        if (!max && end > 1) {
          forcedOverflow = (isViewport ? _doc7.scrollingElement || _docEl2 : scroller).style;
          forcedOverflow = {
            style: forcedOverflow,
            value: forcedOverflow["overflow" + direction.a.toUpperCase()]
          };
          if (isViewport && _getComputedStyle(_body4)["overflow" + direction.a.toUpperCase()] !== "scroll") {
            forcedOverflow.style["overflow" + direction.a.toUpperCase()] = "scroll";
          }
        }
        _swapPinIn(pin, spacer, cs2);
        pinState = _getState(pin);
        bounds = _getBounds(pin, true);
        oppositeScroll = useFixedPosition && _getScrollFunc(scroller, isVertical ? _horizontal : _vertical)();
        if (pinSpacing) {
          spacerState = [pinSpacing + direction.os2, change + otherPinOffset + _px];
          spacerState.t = spacer;
          i2 = pinSpacing === _padding ? _getSize3(pin, direction) + change + otherPinOffset : 0;
          if (i2) {
            spacerState.push(direction.d, i2 + _px);
            spacer.style.flexBasis !== "auto" && (spacer.style.flexBasis = i2 + _px);
          }
          _setState(spacerState);
          if (pinnedContainer) {
            _triggers.forEach(function(t) {
              if (t.pin === pinnedContainer && t.vars.pinSpacing !== false) {
                t._subPinOffset = true;
              }
            });
          }
          useFixedPosition && scrollFunc(prevScroll);
        } else {
          i2 = _getSize3(pin, direction);
          i2 && spacer.style.flexBasis !== "auto" && (spacer.style.flexBasis = i2 + _px);
        }
        if (useFixedPosition) {
          override = {
            top: bounds.top + (isVertical ? scroll - start : oppositeScroll) + _px,
            left: bounds.left + (isVertical ? oppositeScroll : scroll - start) + _px,
            boxSizing: "border-box",
            position: "fixed"
          };
          override[_width] = override["max" + _Width] = Math.ceil(bounds.width) + _px;
          override[_height] = override["max" + _Height] = Math.ceil(bounds.height) + _px;
          override[_margin] = override[_margin + _Top] = override[_margin + _Right] = override[_margin + _Bottom] = override[_margin + _Left] = "0";
          override[_padding] = cs2[_padding];
          override[_padding + _Top] = cs2[_padding + _Top];
          override[_padding + _Right] = cs2[_padding + _Right];
          override[_padding + _Bottom] = cs2[_padding + _Bottom];
          override[_padding + _Left] = cs2[_padding + _Left];
          pinActiveState = _copyState(pinOriginalState, override, pinReparent);
          _refreshingAll && scrollFunc(0);
        }
        if (animation) {
          initted = animation._initted;
          _suppressOverwrites2(1);
          animation.render(animation.duration(), true, true);
          pinChange = pinGetter(direction.a) - pinStart + change + otherPinOffset;
          pinMoves = Math.abs(change - pinChange) > 1;
          useFixedPosition && pinMoves && pinActiveState.splice(pinActiveState.length - 2, 2);
          animation.render(0, true, true);
          initted || animation.invalidate(true);
          animation.parent || animation.totalTime(animation.totalTime());
          _suppressOverwrites2(0);
        } else {
          pinChange = change;
        }
        forcedOverflow && (forcedOverflow.value ? forcedOverflow.style["overflow" + direction.a.toUpperCase()] = forcedOverflow.value : forcedOverflow.style.removeProperty("overflow-" + direction.a));
      } else if (trigger && scrollFunc() && !containerAnimation) {
        bounds = trigger.parentNode;
        while (bounds && bounds !== _body4) {
          if (bounds._pinOffset) {
            start -= bounds._pinOffset;
            end -= bounds._pinOffset;
          }
          bounds = bounds.parentNode;
        }
      }
      revertedPins && revertedPins.forEach(function(t) {
        return t.revert(false, true);
      });
      self.start = start;
      self.end = end;
      scroll1 = scroll2 = _refreshingAll ? prevScroll : scrollFunc();
      if (!containerAnimation && !_refreshingAll) {
        scroll1 < prevScroll && scrollFunc(prevScroll);
        self.scroll.rec = 0;
      }
      self.revert(false, true);
      lastRefresh = _getTime2();
      if (snapDelayedCall) {
        lastSnap = -1;
        snapDelayedCall.restart(true);
      }
      _refreshing = 0;
      animation && isToggle && (animation._initted || prevAnimProgress) && animation.progress() !== prevAnimProgress && animation.progress(prevAnimProgress || 0, true).render(animation.time(), true, true);
      if (isFirstRefresh || prevProgress !== self.progress || containerAnimation || invalidateOnRefresh || animation && !animation._initted) {
        animation && !isToggle && (animation._initted || prevProgress || animation.vars.immediateRender !== false) && animation.totalProgress(containerAnimation && start < -1e-3 && !prevProgress ? gsap16.utils.normalize(start, end, 0) : prevProgress, true);
        self.progress = isFirstRefresh || (scroll1 - start) / change === prevProgress ? 0 : prevProgress;
      }
      pin && pinSpacing && (spacer._pinOffset = Math.round(self.progress * pinChange));
      scrubTween && scrubTween.invalidate();
      if (!isNaN(markerStartOffset)) {
        markerStartOffset -= gsap16.getProperty(markerStartTrigger, direction.p);
        markerEndOffset -= gsap16.getProperty(markerEndTrigger, direction.p);
        _shiftMarker(markerStartTrigger, direction, markerStartOffset);
        _shiftMarker(markerStart, direction, markerStartOffset - (pinOffset || 0));
        _shiftMarker(markerEndTrigger, direction, markerEndOffset);
        _shiftMarker(markerEnd, direction, markerEndOffset - (pinOffset || 0));
      }
      isFirstRefresh && !_refreshingAll && self.update();
      if (onRefresh && !_refreshingAll && !executingOnRefresh) {
        executingOnRefresh = true;
        onRefresh(self);
        executingOnRefresh = false;
      }
    };
    self.getVelocity = function() {
      return (scrollFunc() - scroll2) / (_getTime2() - _time2) * 1e3 || 0;
    };
    self.endAnimation = function() {
      _endAnimation(self.callbackAnimation);
      if (animation) {
        scrubTween ? scrubTween.progress(1) : !animation.paused() ? _endAnimation(animation, animation.reversed()) : isToggle || _endAnimation(animation, self.direction < 0, 1);
      }
    };
    self.labelToScroll = function(label) {
      return animation && animation.labels && (start || self.refresh() || start) + animation.labels[label] / animation.duration() * change || 0;
    };
    self.getTrailing = function(name) {
      var i2 = _triggers.indexOf(self), a = self.direction > 0 ? _triggers.slice(0, i2).reverse() : _triggers.slice(i2 + 1);
      return (_isString5(name) ? a.filter(function(t) {
        return t.vars.preventOverlaps === name;
      }) : a).filter(function(t) {
        return self.direction > 0 ? t.end <= start : t.start >= end;
      });
    };
    self.update = function(reset, recordVelocity, forceFake) {
      if (containerAnimation && !forceFake && !reset) {
        return;
      }
      var scroll = _refreshingAll === true ? prevScroll : self.scroll(), p2 = reset ? 0 : (scroll - start) / change, clipped = p2 < 0 ? 0 : p2 > 1 ? 1 : p2 || 0, prevProgress2 = self.progress, isActive, wasActive, toggleState, action, stateChanged, toggled, isAtMax, isTakingAction;
      if (recordVelocity) {
        scroll2 = scroll1;
        scroll1 = containerAnimation ? scrollFunc() : scroll;
        if (snap3) {
          snap22 = snap1;
          snap1 = animation && !isToggle ? animation.totalProgress() : clipped;
        }
      }
      if (anticipatePin && pin && !_refreshing && !_startup2 && _lastScrollTime) {
        if (!clipped && start < scroll + (scroll - scroll2) / (_getTime2() - _time2) * anticipatePin) {
          clipped = 1e-4;
        } else if (clipped === 1 && end > scroll + (scroll - scroll2) / (_getTime2() - _time2) * anticipatePin) {
          clipped = 0.9999;
        }
      }
      if (clipped !== prevProgress2 && self.enabled) {
        isActive = self.isActive = !!clipped && clipped < 1;
        wasActive = !!prevProgress2 && prevProgress2 < 1;
        toggled = isActive !== wasActive;
        stateChanged = toggled || !!clipped !== !!prevProgress2;
        self.direction = clipped > prevProgress2 ? 1 : -1;
        self.progress = clipped;
        if (stateChanged && !_refreshing) {
          toggleState = clipped && !prevProgress2 ? 0 : clipped === 1 ? 1 : prevProgress2 === 1 ? 2 : 3;
          if (isToggle) {
            action = !toggled && toggleActions[toggleState + 1] !== "none" && toggleActions[toggleState + 1] || toggleActions[toggleState];
            isTakingAction = animation && (action === "complete" || action === "reset" || action in animation);
          }
        }
        preventOverlaps && (toggled || isTakingAction) && (isTakingAction || scrub || !animation) && (_isFunction5(preventOverlaps) ? preventOverlaps(self) : self.getTrailing(preventOverlaps).forEach(function(t) {
          return t.endAnimation();
        }));
        if (!isToggle) {
          if (scrubTween && !_refreshing && !_startup2) {
            scrubTween._dp._time - scrubTween._start !== scrubTween._time && scrubTween.render(scrubTween._dp._time - scrubTween._start);
            if (scrubTween.resetTo) {
              scrubTween.resetTo("totalProgress", clipped, animation._tTime / animation._tDur);
            } else {
              scrubTween.vars.totalProgress = clipped;
              scrubTween.invalidate().restart();
            }
          } else if (animation) {
            animation.totalProgress(clipped, !!(_refreshing && (lastRefresh || reset)));
          }
        }
        if (pin) {
          reset && pinSpacing && (spacer.style[pinSpacing + direction.os2] = spacingStart);
          if (!useFixedPosition) {
            pinSetter(_round17(pinStart + pinChange * clipped));
          } else if (stateChanged) {
            isAtMax = !reset && clipped > prevProgress2 && end + 1 > scroll && scroll + 1 >= _maxScroll(scroller, direction);
            if (pinReparent) {
              if (!reset && (isActive || isAtMax)) {
                var bounds = _getBounds(pin, true), _offset = scroll - start;
                _reparent(pin, _body4, bounds.top + (direction === _vertical ? _offset : 0) + _px, bounds.left + (direction === _vertical ? 0 : _offset) + _px);
              } else {
                _reparent(pin, spacer);
              }
            }
            _setState(isActive || isAtMax ? pinActiveState : pinState);
            pinMoves && clipped < 1 && isActive || pinSetter(pinStart + (clipped === 1 && !isAtMax ? pinChange : 0));
          }
        }
        snap3 && !tweenTo.tween && !_refreshing && !_startup2 && snapDelayedCall.restart(true);
        toggleClass && (toggled || once && clipped && (clipped < 1 || !_limitCallbacks)) && _toArray6(toggleClass.targets).forEach(function(el) {
          return el.classList[isActive || once ? "add" : "remove"](toggleClass.className);
        });
        onUpdate && !isToggle && !reset && onUpdate(self);
        if (stateChanged && !_refreshing) {
          if (isToggle) {
            if (isTakingAction) {
              if (action === "complete") {
                animation.pause().totalProgress(1);
              } else if (action === "reset") {
                animation.restart(true).pause();
              } else if (action === "restart") {
                animation.restart(true);
              } else {
                animation[action]();
              }
            }
            onUpdate && onUpdate(self);
          }
          if (toggled || !_limitCallbacks) {
            onToggle && toggled && _callback3(self, onToggle);
            callbacks[toggleState] && _callback3(self, callbacks[toggleState]);
            once && (clipped === 1 ? self.kill(false, 1) : callbacks[toggleState] = 0);
            if (!toggled) {
              toggleState = clipped === 1 ? 1 : 3;
              callbacks[toggleState] && _callback3(self, callbacks[toggleState]);
            }
          }
          if (fastScrollEnd && !isActive && Math.abs(self.getVelocity()) > (_isNumber5(fastScrollEnd) ? fastScrollEnd : 2500)) {
            _endAnimation(self.callbackAnimation);
            scrubTween ? scrubTween.progress(1) : _endAnimation(animation, action === "reverse" ? 1 : !clipped, 1);
          }
        } else if (isToggle && onUpdate && !_refreshing) {
          onUpdate(self);
        }
      }
      if (markerEndSetter) {
        var n = containerAnimation ? scroll / containerAnimation.duration() * (containerAnimation._caScrollDist || 0) : scroll;
        markerStartSetter(n + (markerStartTrigger._isFlipped ? 1 : 0));
        markerEndSetter(n);
      }
      caMarkerSetter && caMarkerSetter(-scroll / containerAnimation.duration() * (containerAnimation._caScrollDist || 0));
    };
    self.enable = function(reset, refresh) {
      if (!self.enabled) {
        self.enabled = true;
        _addListener3(scroller, "resize", _onResize);
        isViewport || _addListener3(scroller, "scroll", _onScroll3);
        onRefreshInit && _addListener3(ScrollTrigger5, "refreshInit", onRefreshInit);
        if (reset !== false) {
          self.progress = prevProgress = 0;
          scroll1 = scroll2 = lastSnap = scrollFunc();
        }
        refresh !== false && self.refresh();
      }
    };
    self.getTween = function(snap4) {
      return snap4 && tweenTo ? tweenTo.tween : scrubTween;
    };
    self.setPositions = function(newStart, newEnd, keepClamp, pinOffset) {
      if (containerAnimation) {
        var st = containerAnimation.scrollTrigger, duration = containerAnimation.duration(), _change = st.end - st.start;
        newStart = st.start + _change * newStart / duration;
        newEnd = st.start + _change * newEnd / duration;
      }
      self.refresh(false, false, {
        start: _keepClamp(newStart, keepClamp && !!self._startClamp),
        end: _keepClamp(newEnd, keepClamp && !!self._endClamp)
      }, pinOffset);
      self.update();
    };
    self.adjustPinSpacing = function(amount) {
      if (spacerState && amount) {
        var i2 = spacerState.indexOf(direction.d) + 1;
        spacerState[i2] = parseFloat(spacerState[i2]) + amount + _px;
        spacerState[1] = parseFloat(spacerState[1]) + amount + _px;
        _setState(spacerState);
      }
    };
    self.disable = function(reset, allowAnimation) {
      reset !== false && self.revert(true, true);
      if (self.enabled) {
        self.enabled = self.isActive = false;
        allowAnimation || scrubTween && scrubTween.pause();
        prevScroll = 0;
        pinCache && (pinCache.uncache = 1);
        onRefreshInit && _removeListener3(ScrollTrigger5, "refreshInit", onRefreshInit);
        if (snapDelayedCall) {
          snapDelayedCall.pause();
          tweenTo.tween && tweenTo.tween.kill() && (tweenTo.tween = 0);
        }
        if (!isViewport) {
          var i2 = _triggers.length;
          while (i2--) {
            if (_triggers[i2].scroller === scroller && _triggers[i2] !== self) {
              return;
            }
          }
          _removeListener3(scroller, "resize", _onResize);
          isViewport || _removeListener3(scroller, "scroll", _onScroll3);
        }
      }
    };
    self.kill = function(revert, allowAnimation) {
      self.disable(revert, allowAnimation);
      scrubTween && !allowAnimation && scrubTween.kill();
      id && delete _ids[id];
      var i2 = _triggers.indexOf(self);
      i2 >= 0 && _triggers.splice(i2, 1);
      i2 === _i && _direction > 0 && _i--;
      i2 = 0;
      _triggers.forEach(function(t) {
        return t.scroller === self.scroller && (i2 = 1);
      });
      i2 || _refreshingAll || (self.scroll.rec = 0);
      if (animation) {
        animation.scrollTrigger = null;
        revert && animation.revert({
          kill: false
        });
        allowAnimation || animation.kill();
      }
      markerStart && [markerStart, markerEnd, markerStartTrigger, markerEndTrigger].forEach(function(m) {
        return m.parentNode && m.parentNode.removeChild(m);
      });
      _primary === self && (_primary = 0);
      if (pin) {
        pinCache && (pinCache.uncache = 1);
        i2 = 0;
        _triggers.forEach(function(t) {
          return t.pin === pin && i2++;
        });
        i2 || (pinCache.spacer = 0);
      }
      vars.onKill && vars.onKill(self);
    };
    _triggers.push(self);
    self.enable(false, false);
    customRevertReturn && customRevertReturn(self);
    if (animation && animation.add && !change) {
      var updateFunc = self.update;
      self.update = function() {
        self.update = updateFunc;
        _scrollers.cache++;
        start || end || self.refresh();
      };
      gsap16.delayedCall(0.01, self.update);
      change = 0.01;
      start = end = 0;
    } else {
      self.refresh();
    }
    pin && _queueRefreshAll();
  };
  ScrollTrigger5.register = function register8(core) {
    if (!_coreInitted13) {
      gsap16 = core || _getGSAP25();
      _windowExists7() && window.document && ScrollTrigger5.enable();
      _coreInitted13 = _enabled;
    }
    return _coreInitted13;
  };
  ScrollTrigger5.defaults = function defaults2(config3) {
    if (config3) {
      for (var p2 in config3) {
        _defaults2[p2] = config3[p2];
      }
    }
    return _defaults2;
  };
  ScrollTrigger5.disable = function disable(reset, kill5) {
    _enabled = 0;
    _triggers.forEach(function(trigger) {
      return trigger[kill5 ? "kill" : "disable"](reset);
    });
    _removeListener3(_win6, "wheel", _onScroll3);
    _removeListener3(_doc7, "scroll", _onScroll3);
    clearInterval(_syncInterval);
    _removeListener3(_doc7, "touchcancel", _passThrough3);
    _removeListener3(_body4, "touchstart", _passThrough3);
    _multiListener(_removeListener3, _doc7, "pointerdown,touchstart,mousedown", _pointerDownHandler);
    _multiListener(_removeListener3, _doc7, "pointerup,touchend,mouseup", _pointerUpHandler);
    _resizeDelay.kill();
    _iterateAutoRefresh(_removeListener3);
    for (var i2 = 0; i2 < _scrollers.length; i2 += 3) {
      _wheelListener(_removeListener3, _scrollers[i2], _scrollers[i2 + 1]);
      _wheelListener(_removeListener3, _scrollers[i2], _scrollers[i2 + 2]);
    }
  };
  ScrollTrigger5.enable = function enable() {
    _win6 = window;
    _doc7 = document;
    _docEl2 = _doc7.documentElement;
    _body4 = _doc7.body;
    if (gsap16) {
      _toArray6 = gsap16.utils.toArray;
      _clamp4 = gsap16.utils.clamp;
      _context4 = gsap16.core.context || _passThrough3;
      _suppressOverwrites2 = gsap16.core.suppressOverwrites || _passThrough3;
      _scrollRestoration = _win6.history.scrollRestoration || "auto";
      _lastScroll = _win6.pageYOffset || 0;
      gsap16.core.globals("ScrollTrigger", ScrollTrigger5);
      if (_body4) {
        _enabled = 1;
        _div100vh = document.createElement("div");
        _div100vh.style.height = "100vh";
        _div100vh.style.position = "absolute";
        _refresh100vh();
        _rafBugFix();
        Observer.register(gsap16);
        ScrollTrigger5.isTouch = Observer.isTouch;
        _fixIOSBug = Observer.isTouch && /(iPad|iPhone|iPod|Mac)/g.test(navigator.userAgent);
        _ignoreMobileResize = Observer.isTouch === 1;
        _addListener3(_win6, "wheel", _onScroll3);
        _root2 = [_win6, _doc7, _docEl2, _body4];
        if (gsap16.matchMedia) {
          ScrollTrigger5.matchMedia = function(vars) {
            var mm = gsap16.matchMedia(), p2;
            for (p2 in vars) {
              mm.add(p2, vars[p2]);
            }
            return mm;
          };
          gsap16.addEventListener("matchMediaInit", function() {
            _recordScrollPositions();
            _revertAll();
          });
          gsap16.addEventListener("matchMediaRevert", function() {
            return _revertRecorded();
          });
          gsap16.addEventListener("matchMedia", function() {
            _refreshAll(0, 1);
            _dispatch3("matchMedia");
          });
          gsap16.matchMedia().add("(orientation: portrait)", function() {
            _setBaseDimensions();
            return _setBaseDimensions;
          });
        } else {
          console.warn("Requires GSAP 3.11.0 or later");
        }
        _setBaseDimensions();
        _addListener3(_doc7, "scroll", _onScroll3);
        var bodyHasStyle = _body4.hasAttribute("style"), bodyStyle = _body4.style, border = bodyStyle.borderTopStyle, AnimationProto = gsap16.core.Animation.prototype, bounds, i2;
        AnimationProto.revert || Object.defineProperty(AnimationProto, "revert", {
          value: function value() {
            return this.time(-0.01, true);
          }
        });
        bodyStyle.borderTopStyle = "solid";
        bounds = _getBounds(_body4);
        _vertical.m = Math.round(bounds.top + _vertical.sc()) || 0;
        _horizontal.m = Math.round(bounds.left + _horizontal.sc()) || 0;
        border ? bodyStyle.borderTopStyle = border : bodyStyle.removeProperty("border-top-style");
        if (!bodyHasStyle) {
          _body4.setAttribute("style", "");
          _body4.removeAttribute("style");
        }
        _syncInterval = setInterval(_sync, 250);
        gsap16.delayedCall(0.5, function() {
          return _startup2 = 0;
        });
        _addListener3(_doc7, "touchcancel", _passThrough3);
        _addListener3(_body4, "touchstart", _passThrough3);
        _multiListener(_addListener3, _doc7, "pointerdown,touchstart,mousedown", _pointerDownHandler);
        _multiListener(_addListener3, _doc7, "pointerup,touchend,mouseup", _pointerUpHandler);
        _transformProp3 = gsap16.utils.checkPrefix("transform");
        _stateProps.push(_transformProp3);
        _coreInitted13 = _getTime2();
        _resizeDelay = gsap16.delayedCall(0.2, _refreshAll).pause();
        _autoRefresh = [_doc7, "visibilitychange", function() {
          var w = _win6.innerWidth, h = _win6.innerHeight;
          if (_doc7.hidden) {
            _prevWidth = w;
            _prevHeight = h;
          } else if (_prevWidth !== w || _prevHeight !== h) {
            _onResize();
          }
        }, _doc7, "DOMContentLoaded", _refreshAll, _win6, "load", _refreshAll, _win6, "resize", _onResize];
        _iterateAutoRefresh(_addListener3);
        _triggers.forEach(function(trigger) {
          return trigger.enable(0, 1);
        });
        for (i2 = 0; i2 < _scrollers.length; i2 += 3) {
          _wheelListener(_removeListener3, _scrollers[i2], _scrollers[i2 + 1]);
          _wheelListener(_removeListener3, _scrollers[i2], _scrollers[i2 + 2]);
        }
      } else if (_doc7) {
        var onLoad = function onLoad2() {
          ScrollTrigger5.enable();
          _doc7.removeEventListener("DOMContentLoaded", onLoad2);
        };
        _doc7.addEventListener("DOMContentLoaded", onLoad);
      }
    }
  };
  ScrollTrigger5.config = function config3(vars) {
    "limitCallbacks" in vars && (_limitCallbacks = !!vars.limitCallbacks);
    var ms = vars.syncInterval;
    ms && clearInterval(_syncInterval) || (_syncInterval = ms) && setInterval(_sync, ms);
    "ignoreMobileResize" in vars && (_ignoreMobileResize = ScrollTrigger5.isTouch === 1 && vars.ignoreMobileResize);
    if ("autoRefreshEvents" in vars) {
      _iterateAutoRefresh(_removeListener3) || _iterateAutoRefresh(_addListener3, vars.autoRefreshEvents || "none");
      _ignoreResize = (vars.autoRefreshEvents + "").indexOf("resize") === -1;
    }
  };
  ScrollTrigger5.scrollerProxy = function scrollerProxy(target, vars) {
    var t = _getTarget(target), i2 = _scrollers.indexOf(t), isViewport = _isViewport3(t);
    if (~i2) {
      _scrollers.splice(i2, isViewport ? 6 : 2);
    }
    if (vars) {
      isViewport ? _proxies.unshift(_win6, vars, _body4, vars, _docEl2, vars) : _proxies.unshift(t, vars);
    }
  };
  ScrollTrigger5.clearMatchMedia = function clearMatchMedia(query) {
    _triggers.forEach(function(t) {
      return t._ctx && t._ctx.query === query && t._ctx.kill(true, true);
    });
  };
  ScrollTrigger5.isInViewport = function isInViewport(element, ratio, horizontal) {
    var bounds = (_isString5(element) ? _getTarget(element) : element).getBoundingClientRect(), offset = bounds[horizontal ? _width : _height] * ratio || 0;
    return horizontal ? bounds.right - offset > 0 && bounds.left + offset < _win6.innerWidth : bounds.bottom - offset > 0 && bounds.top + offset < _win6.innerHeight;
  };
  ScrollTrigger5.positionInViewport = function positionInViewport(element, referencePoint, horizontal) {
    _isString5(element) && (element = _getTarget(element));
    var bounds = element.getBoundingClientRect(), size = bounds[horizontal ? _width : _height], offset = referencePoint == null ? size / 2 : referencePoint in _keywords ? _keywords[referencePoint] * size : ~referencePoint.indexOf("%") ? parseFloat(referencePoint) * size / 100 : parseFloat(referencePoint) || 0;
    return horizontal ? (bounds.left + offset) / _win6.innerWidth : (bounds.top + offset) / _win6.innerHeight;
  };
  ScrollTrigger5.killAll = function killAll(allowListeners) {
    _triggers.slice(0).forEach(function(t) {
      return t.vars.id !== "ScrollSmoother" && t.kill();
    });
    if (allowListeners !== true) {
      var listeners = _listeners2.killAll || [];
      _listeners2 = {};
      listeners.forEach(function(f) {
        return f();
      });
    }
  };
  return ScrollTrigger5;
})();
ScrollTrigger2.version = "3.15.0";
ScrollTrigger2.saveStyles = function(targets) {
  return targets ? _toArray6(targets).forEach(function(target) {
    if (target && target.style) {
      var i2 = _savedStyles.indexOf(target);
      i2 >= 0 && _savedStyles.splice(i2, 5);
      _savedStyles.push(target, target.style.cssText, target.getBBox && target.getAttribute("transform"), gsap16.core.getCache(target), _context4());
    }
  }) : _savedStyles;
};
ScrollTrigger2.revert = function(soft, media) {
  return _revertAll(!soft, media);
};
ScrollTrigger2.create = function(vars, animation) {
  return new ScrollTrigger2(vars, animation);
};
ScrollTrigger2.refresh = function(safe) {
  return safe ? _onResize(true) : (_coreInitted13 || ScrollTrigger2.register()) && _refreshAll(true);
};
ScrollTrigger2.update = function(force) {
  return ++_scrollers.cache && _updateAll(force === true ? 2 : 0);
};
ScrollTrigger2.clearScrollMemory = _clearScrollMemory;
ScrollTrigger2.maxScroll = function(element, horizontal) {
  return _maxScroll(element, horizontal ? _horizontal : _vertical);
};
ScrollTrigger2.getScrollFunc = function(element, horizontal) {
  return _getScrollFunc(_getTarget(element), horizontal ? _horizontal : _vertical);
};
ScrollTrigger2.getById = function(id) {
  return _ids[id];
};
ScrollTrigger2.getAll = function() {
  return _triggers.filter(function(t) {
    return t.vars.id !== "ScrollSmoother";
  });
};
ScrollTrigger2.isScrolling = function() {
  return !!_lastScrollTime;
};
ScrollTrigger2.snapDirectional = _snapDirectional;
ScrollTrigger2.addEventListener = function(type, callback) {
  var a = _listeners2[type] || (_listeners2[type] = []);
  ~a.indexOf(callback) || a.push(callback);
};
ScrollTrigger2.removeEventListener = function(type, callback) {
  var a = _listeners2[type], i2 = a && a.indexOf(callback);
  i2 >= 0 && a.splice(i2, 1);
};
ScrollTrigger2.batch = function(targets, vars) {
  var result = [], varsCopy = {}, interval = vars.interval || 0.016, batchMax = vars.batchMax || 1e9, proxyCallback = function proxyCallback2(type, callback) {
    var elements = [], triggers = [], delay = gsap16.delayedCall(interval, function() {
      callback(elements, triggers);
      elements = [];
      triggers = [];
    }).pause();
    return function(self) {
      elements.length || delay.restart(true);
      elements.push(self.trigger);
      triggers.push(self);
      batchMax <= elements.length && delay.progress(1);
    };
  }, p2;
  for (p2 in vars) {
    varsCopy[p2] = p2.substr(0, 2) === "on" && _isFunction5(vars[p2]) && p2 !== "onRefreshInit" ? proxyCallback(p2, vars[p2]) : vars[p2];
  }
  if (_isFunction5(batchMax)) {
    batchMax = batchMax();
    _addListener3(ScrollTrigger2, "refresh", function() {
      return batchMax = vars.batchMax();
    });
  }
  _toArray6(targets).forEach(function(target) {
    var config3 = {};
    for (p2 in varsCopy) {
      config3[p2] = varsCopy[p2];
    }
    config3.trigger = target;
    result.push(ScrollTrigger2.create(config3));
  });
  return result;
};
var _clampScrollAndGetDurationMultiplier = function _clampScrollAndGetDurationMultiplier2(scrollFunc, current, end, max) {
  current > max ? scrollFunc(max) : current < 0 && scrollFunc(0);
  return end > max ? (max - current) / (end - current) : end < 0 ? current / (current - end) : 1;
};
var _allowNativePanning = function _allowNativePanning2(target, direction) {
  if (direction === true) {
    target.style.removeProperty("touch-action");
  } else {
    target.style.touchAction = direction === true ? "auto" : direction ? "pan-" + direction + (Observer.isTouch ? " pinch-zoom" : "") : "none";
  }
  target === _docEl2 && _allowNativePanning2(_body4, direction);
};
var _overflow = {
  auto: 1,
  scroll: 1
};
var _nestedScroll = function _nestedScroll2(_ref5) {
  var event = _ref5.event, target = _ref5.target, axis = _ref5.axis;
  var node = (event.changedTouches ? event.changedTouches[0] : event).target, cache = node._gsap || gsap16.core.getCache(node), time = _getTime2(), cs;
  if (!cache._isScrollT || time - cache._isScrollT > 2e3) {
    while (node && node !== _body4 && (node.scrollHeight <= node.clientHeight && node.scrollWidth <= node.clientWidth || !(_overflow[(cs = _getComputedStyle(node)).overflowY] || _overflow[cs.overflowX]))) {
      node = node.parentNode;
    }
    cache._isScroll = node && node !== target && !_isViewport3(node) && (_overflow[(cs = _getComputedStyle(node)).overflowY] || _overflow[cs.overflowX]);
    cache._isScrollT = time;
  }
  if (cache._isScroll || axis === "x") {
    event.stopPropagation();
    event._gsapAllow = true;
  }
};
var _inputObserver = function _inputObserver2(target, type, inputs, nested) {
  return Observer.create({
    target,
    capture: true,
    debounce: false,
    lockAxis: true,
    type,
    onWheel: nested = nested && _nestedScroll,
    onPress: nested,
    onDrag: nested,
    onScroll: nested,
    onEnable: function onEnable() {
      return inputs && _addListener3(_doc7, Observer.eventTypes[0], _captureInputs, false, true);
    },
    onDisable: function onDisable() {
      return _removeListener3(_doc7, Observer.eventTypes[0], _captureInputs, true);
    }
  });
};
var _inputExp = /(input|label|select|textarea)/i;
var _inputIsFocused;
var _captureInputs = function _captureInputs2(e) {
  var isInput = _inputExp.test(e.target.tagName);
  if (isInput || _inputIsFocused) {
    e._gsapAllow = true;
    _inputIsFocused = isInput;
  }
};
var _getScrollNormalizer = function _getScrollNormalizer2(vars) {
  _isObject3(vars) || (vars = {});
  vars.preventDefault = vars.isNormalizer = vars.allowClicks = true;
  vars.type || (vars.type = "wheel,touch");
  vars.debounce = !!vars.debounce;
  vars.id = vars.id || "normalizer";
  var _vars2 = vars, normalizeScrollX = _vars2.normalizeScrollX, momentum = _vars2.momentum, allowNestedScroll = _vars2.allowNestedScroll, onRelease = _vars2.onRelease, self, maxY, target = _getTarget(vars.target) || _docEl2, smoother = gsap16.core.globals().ScrollSmoother, smootherInstance = smoother && smoother.get(), content = _fixIOSBug && (vars.content && _getTarget(vars.content) || smootherInstance && vars.content !== false && !smootherInstance.smooth() && smootherInstance.content()), scrollFuncY = _getScrollFunc(target, _vertical), scrollFuncX = _getScrollFunc(target, _horizontal), scale = 1, initialScale = (Observer.isTouch && _win6.visualViewport ? _win6.visualViewport.scale * _win6.visualViewport.width : _win6.outerWidth) / _win6.innerWidth, wheelRefresh = 0, resolveMomentumDuration = _isFunction5(momentum) ? function() {
    return momentum(self);
  } : function() {
    return momentum || 2.8;
  }, lastRefreshID, skipTouchMove, inputObserver = _inputObserver(target, vars.type, true, allowNestedScroll), resumeTouchMove = function resumeTouchMove2() {
    return skipTouchMove = false;
  }, scrollClampX = _passThrough3, scrollClampY = _passThrough3, updateClamps = function updateClamps2() {
    maxY = _maxScroll(target, _vertical);
    scrollClampY = _clamp4(_fixIOSBug ? 1 : 0, maxY);
    normalizeScrollX && (scrollClampX = _clamp4(0, _maxScroll(target, _horizontal)));
    lastRefreshID = _refreshID;
  }, removeContentOffset = function removeContentOffset2() {
    content._gsap.y = _round17(parseFloat(content._gsap.y) + scrollFuncY.offset) + "px";
    content.style.transform = "matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, " + parseFloat(content._gsap.y) + ", 0, 1)";
    scrollFuncY.offset = scrollFuncY.cacheID = 0;
  }, ignoreDrag = function ignoreDrag2() {
    if (skipTouchMove) {
      requestAnimationFrame(resumeTouchMove);
      var offset = _round17(self.deltaY / 2), scroll = scrollClampY(scrollFuncY.v - offset);
      if (content && scroll !== scrollFuncY.v + scrollFuncY.offset) {
        scrollFuncY.offset = scroll - scrollFuncY.v;
        var y = _round17((parseFloat(content && content._gsap.y) || 0) - scrollFuncY.offset);
        content.style.transform = "matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, " + y + ", 0, 1)";
        content._gsap.y = y + "px";
        scrollFuncY.cacheID = _scrollers.cache;
        _updateAll();
      }
      return true;
    }
    scrollFuncY.offset && removeContentOffset();
    skipTouchMove = true;
  }, tween, startScrollX, startScrollY, onStopDelayedCall, onResize = function onResize2() {
    updateClamps();
    if (tween.isActive() && tween.vars.scrollY > maxY) {
      scrollFuncY() > maxY ? tween.progress(1) && scrollFuncY(maxY) : tween.resetTo("scrollY", maxY);
    }
  };
  content && gsap16.set(content, {
    y: "+=0"
  });
  vars.ignoreCheck = function(e) {
    return _fixIOSBug && e.type === "touchmove" && ignoreDrag(e) || scale > 1.05 && e.type !== "touchstart" || self.isGesturing || e.touches && e.touches.length > 1;
  };
  vars.onPress = function() {
    skipTouchMove = false;
    var prevScale = scale;
    scale = _round17((_win6.visualViewport && _win6.visualViewport.scale || 1) / initialScale);
    tween.pause();
    prevScale !== scale && _allowNativePanning(target, scale > 1.01 ? true : normalizeScrollX ? false : "x");
    startScrollX = scrollFuncX();
    startScrollY = scrollFuncY();
    updateClamps();
    lastRefreshID = _refreshID;
  };
  vars.onRelease = vars.onGestureStart = function(self2, wasDragging) {
    scrollFuncY.offset && removeContentOffset();
    if (!wasDragging) {
      onStopDelayedCall.restart(true);
    } else {
      _scrollers.cache++;
      var dur = resolveMomentumDuration(), currentScroll, endScroll;
      if (normalizeScrollX) {
        currentScroll = scrollFuncX();
        endScroll = currentScroll + dur * 0.05 * -self2.velocityX / 0.227;
        dur *= _clampScrollAndGetDurationMultiplier(scrollFuncX, currentScroll, endScroll, _maxScroll(target, _horizontal));
        tween.vars.scrollX = scrollClampX(endScroll);
      }
      currentScroll = scrollFuncY();
      endScroll = currentScroll + dur * 0.05 * -self2.velocityY / 0.227;
      dur *= _clampScrollAndGetDurationMultiplier(scrollFuncY, currentScroll, endScroll, _maxScroll(target, _vertical));
      tween.vars.scrollY = scrollClampY(endScroll);
      tween.invalidate().duration(dur).play(0.01);
      if (_fixIOSBug && tween.vars.scrollY >= maxY || currentScroll >= maxY - 1) {
        gsap16.to({}, {
          onUpdate: onResize,
          duration: dur
        });
      }
    }
    onRelease && onRelease(self2);
  };
  vars.onWheel = function() {
    tween._ts && tween.pause();
    if (_getTime2() - wheelRefresh > 1e3) {
      lastRefreshID = 0;
      wheelRefresh = _getTime2();
    }
  };
  vars.onChange = function(self2, dx, dy, xArray, yArray) {
    _refreshID !== lastRefreshID && updateClamps();
    dx && normalizeScrollX && scrollFuncX(scrollClampX(xArray[2] === dx ? startScrollX + (self2.startX - self2.x) : scrollFuncX() + dx - xArray[1]));
    if (dy) {
      scrollFuncY.offset && removeContentOffset();
      var isTouch = yArray[2] === dy, y = isTouch ? startScrollY + self2.startY - self2.y : scrollFuncY() + dy - yArray[1], yClamped = scrollClampY(y);
      isTouch && y !== yClamped && (startScrollY += yClamped - y);
      scrollFuncY(yClamped);
    }
    (dy || dx) && _updateAll();
  };
  vars.onEnable = function() {
    _allowNativePanning(target, normalizeScrollX ? false : "x");
    ScrollTrigger2.addEventListener("refresh", onResize);
    _addListener3(_win6, "resize", onResize);
    if (scrollFuncY.smooth) {
      scrollFuncY.target.style.scrollBehavior = "auto";
      scrollFuncY.smooth = scrollFuncX.smooth = false;
    }
    inputObserver.enable();
  };
  vars.onDisable = function() {
    _allowNativePanning(target, true);
    _removeListener3(_win6, "resize", onResize);
    ScrollTrigger2.removeEventListener("refresh", onResize);
    inputObserver.kill();
  };
  vars.lockAxis = vars.lockAxis !== false;
  self = new Observer(vars);
  self.iOS = _fixIOSBug;
  _fixIOSBug && !scrollFuncY() && scrollFuncY(1);
  _fixIOSBug && gsap16.ticker.add(_passThrough3);
  onStopDelayedCall = self._dc;
  tween = gsap16.to(self, {
    ease: "power4",
    paused: true,
    inherit: false,
    scrollX: normalizeScrollX ? "+=0.1" : "+=0",
    scrollY: "+=0.1",
    modifiers: {
      scrollY: _interruptionTracker(scrollFuncY, scrollFuncY(), function() {
        return tween.pause();
      })
    },
    onUpdate: _updateAll,
    onComplete: onStopDelayedCall.vars.onComplete
  });
  return self;
};
ScrollTrigger2.sort = function(func) {
  if (_isFunction5(func)) {
    return _triggers.sort(func);
  }
  var scroll = _win6.pageYOffset || 0;
  ScrollTrigger2.getAll().forEach(function(t) {
    return t._sortY = t.trigger ? scroll + t.trigger.getBoundingClientRect().top : t.start + _win6.innerHeight;
  });
  return _triggers.sort(func || function(a, b) {
    return (a.vars.refreshPriority || 0) * -1e6 + (a.vars.containerAnimation ? 1e6 : a._sortY) - ((b.vars.containerAnimation ? 1e6 : b._sortY) + (b.vars.refreshPriority || 0) * -1e6);
  });
};
ScrollTrigger2.observe = function(vars) {
  return new Observer(vars);
};
ScrollTrigger2.normalizeScroll = function(vars) {
  if (typeof vars === "undefined") {
    return _normalizer2;
  }
  if (vars === true && _normalizer2) {
    return _normalizer2.enable();
  }
  if (vars === false) {
    _normalizer2 && _normalizer2.kill();
    _normalizer2 = vars;
    return;
  }
  var normalizer = vars instanceof Observer ? vars : _getScrollNormalizer(vars);
  _normalizer2 && _normalizer2.target === normalizer.target && _normalizer2.kill();
  _isViewport3(normalizer.target) && (_normalizer2 = normalizer);
  return normalizer;
};
ScrollTrigger2.core = {
  // smaller file size way to leverage in ScrollSmoother and Observer
  _getVelocityProp,
  _inputObserver,
  _scrollers,
  _proxies,
  bridge: {
    // when normalizeScroll sets the scroll position (ss = setScroll)
    ss: function ss() {
      _lastScrollTime || _dispatch3("scrollStart");
      _lastScrollTime = _getTime2();
    },
    // a way to get the _refreshing value in Observer
    ref: function ref() {
      return _refreshing;
    }
  }
};
_getGSAP25() && gsap16.registerPlugin(ScrollTrigger2);

// examples/figma-launch/node_modules/gsap/ScrollToPlugin.js
var gsap17;
var _coreInitted14;
var _window;
var _docEl3;
var _body5;
var _toArray7;
var _config2;
var ScrollTrigger3;
var _windowExists9 = function _windowExists10() {
  return typeof window !== "undefined";
};
var _getGSAP27 = function _getGSAP28() {
  return gsap17 || _windowExists9() && (gsap17 = window.gsap) && gsap17.registerPlugin && gsap17;
};
var _isString7 = function _isString8(value) {
  return typeof value === "string";
};
var _isFunction7 = function _isFunction8(value) {
  return typeof value === "function";
};
var _max = function _max2(element, axis) {
  var dim = axis === "x" ? "Width" : "Height", scroll = "scroll" + dim, client = "client" + dim;
  return element === _window || element === _docEl3 || element === _body5 ? Math.max(_docEl3[scroll], _body5[scroll]) - (_window["inner" + dim] || _docEl3[client] || _body5[client]) : element[scroll] - element["offset" + dim];
};
var _buildGetter = function _buildGetter2(e, axis) {
  var p2 = "scroll" + (axis === "x" ? "Left" : "Top");
  if (e === _window) {
    if (e.pageXOffset != null) {
      p2 = "page" + axis.toUpperCase() + "Offset";
    } else {
      e = _docEl3[p2] != null ? _docEl3 : _body5;
    }
  }
  return function() {
    return e[p2];
  };
};
var _clean = function _clean2(value, index, target, targets) {
  _isFunction7(value) && (value = value(index, target, targets));
  if (typeof value !== "object") {
    return _isString7(value) && value !== "max" && value.charAt(1) !== "=" ? {
      x: value,
      y: value
    } : {
      y: value
    };
  } else if (value.nodeType) {
    return {
      y: value,
      x: value
    };
  } else {
    var result = {}, p2;
    for (p2 in value) {
      result[p2] = p2 !== "onAutoKill" && _isFunction7(value[p2]) ? value[p2](index, target, targets) : value[p2];
    }
    return result;
  }
};
var _getOffset = function _getOffset2(element, container) {
  element = _toArray7(element)[0];
  if (!element || !element.getBoundingClientRect) {
    return console.warn("scrollTo target doesn't exist. Using 0") || {
      x: 0,
      y: 0
    };
  }
  var rect = element.getBoundingClientRect(), isRoot = !container || container === _window || container === _body5, cRect = isRoot ? {
    top: _docEl3.clientTop - (_window.pageYOffset || _docEl3.scrollTop || _body5.scrollTop || 0),
    left: _docEl3.clientLeft - (_window.pageXOffset || _docEl3.scrollLeft || _body5.scrollLeft || 0)
  } : container.getBoundingClientRect(), offsets = {
    x: rect.left - cRect.left,
    y: rect.top - cRect.top
  };
  if (!isRoot && container) {
    offsets.x += _buildGetter(container, "x")();
    offsets.y += _buildGetter(container, "y")();
  }
  return offsets;
};
var _parseVal = function _parseVal2(value, target, axis, currentVal, offset) {
  return !isNaN(value) && typeof value !== "object" ? parseFloat(value) - offset : _isString7(value) && value.charAt(1) === "=" ? parseFloat(value.substr(2)) * (value.charAt(0) === "-" ? -1 : 1) + currentVal - offset : value === "max" ? _max(target, axis) - offset : Math.min(_max(target, axis), _getOffset(value, target)[axis] - offset);
};
var _initCore23 = function _initCore24() {
  gsap17 = _getGSAP27();
  if (_windowExists9() && gsap17 && typeof document !== "undefined" && document.body) {
    _window = window;
    _body5 = document.body;
    _docEl3 = document.documentElement;
    _toArray7 = gsap17.utils.toArray;
    gsap17.config({
      autoKillThreshold: 7
    });
    _config2 = gsap17.config();
    _coreInitted14 = 1;
  }
};
var ScrollToPlugin = {
  version: "3.15.0",
  name: "scrollTo",
  rawVars: 1,
  register: function register5(core) {
    gsap17 = core;
    _initCore23();
  },
  init: function init11(target, value, tween, index, targets) {
    _coreInitted14 || _initCore23();
    var data = this, snapType = gsap17.getProperty(target, "scrollSnapType");
    data.isWin = target === _window;
    data.target = target;
    data.tween = tween;
    value = _clean(value, index, target, targets);
    data.vars = value;
    data.autoKill = !!("autoKill" in value ? value : _config2).autoKill;
    data.getX = _buildGetter(target, "x");
    data.getY = _buildGetter(target, "y");
    data.x = data.xPrev = data.getX();
    data.y = data.yPrev = data.getY();
    ScrollTrigger3 || (ScrollTrigger3 = gsap17.core.globals().ScrollTrigger);
    gsap17.getProperty(target, "scrollBehavior") === "smooth" && gsap17.set(target, {
      scrollBehavior: "auto"
    });
    if (snapType && snapType !== "none") {
      data.snap = 1;
      data.snapInline = target.style.scrollSnapType;
      target.style.scrollSnapType = "none";
    }
    if (value.x != null) {
      data.add(data, "x", data.x, _parseVal(value.x, target, "x", data.x, value.offsetX || 0), index, targets);
      data._props.push("scrollTo_x");
    } else {
      data.skipX = 1;
    }
    if (value.y != null) {
      data.add(data, "y", data.y, _parseVal(value.y, target, "y", data.y, value.offsetY || 0), index, targets);
      data._props.push("scrollTo_y");
    } else {
      data.skipY = 1;
    }
  },
  render: function render10(ratio, data) {
    var pt = data._pt, target = data.target, tween = data.tween, autoKill = data.autoKill, xPrev = data.xPrev, yPrev = data.yPrev, isWin = data.isWin, snap3 = data.snap, snapInline = data.snapInline, x, y, yDif, xDif, threshold;
    while (pt) {
      pt.r(ratio, pt.d);
      pt = pt._next;
    }
    x = isWin || !data.skipX ? data.getX() : xPrev;
    y = isWin || !data.skipY ? data.getY() : yPrev;
    yDif = y - yPrev;
    xDif = x - xPrev;
    threshold = _config2.autoKillThreshold;
    if (data.x < 0) {
      data.x = 0;
    }
    if (data.y < 0) {
      data.y = 0;
    }
    if (autoKill) {
      if (!data.skipX && (xDif > threshold || xDif < -threshold) && x < _max(target, "x")) {
        data.skipX = 1;
      }
      if (!data.skipY && (yDif > threshold || yDif < -threshold) && y < _max(target, "y")) {
        data.skipY = 1;
      }
      if (data.skipX && data.skipY) {
        tween.kill();
        data.vars.onAutoKill && data.vars.onAutoKill.apply(tween, data.vars.onAutoKillParams || []);
      }
    }
    if (isWin) {
      _window.scrollTo(!data.skipX ? data.x : x, !data.skipY ? data.y : y);
    } else {
      data.skipY || (target.scrollTop = data.y);
      data.skipX || (target.scrollLeft = data.x);
    }
    if (snap3 && (ratio === 1 || ratio === 0)) {
      y = target.scrollTop;
      x = target.scrollLeft;
      snapInline ? target.style.scrollSnapType = snapInline : target.style.removeProperty("scroll-snap-type");
      target.scrollTop = y + 1;
      target.scrollLeft = x + 1;
      target.scrollTop = y;
      target.scrollLeft = x;
    }
    data.xPrev = data.x;
    data.yPrev = data.y;
    ScrollTrigger3 && ScrollTrigger3.update();
  },
  kill: function kill4(property) {
    var both = property === "scrollTo", i2 = this._props.indexOf(property);
    if (both || property === "scrollTo_x") {
      this.skipX = 1;
    }
    if (both || property === "scrollTo_y") {
      this.skipY = 1;
    }
    i2 > -1 && this._props.splice(i2, 1);
    return !this._props.length;
  }
};
ScrollToPlugin.max = _max;
ScrollToPlugin.getOffset = _getOffset;
ScrollToPlugin.buildGetter = _buildGetter;
ScrollToPlugin.config = function(vars) {
  _config2 || _initCore23() || (_config2 = gsap17.config());
  for (var p2 in vars) {
    _config2[p2] = vars[p2];
  }
};
_getGSAP27() && gsap17.registerPlugin(ScrollToPlugin);

// examples/figma-launch/node_modules/gsap/Draggable.js
function _assertThisInitialized2(self) {
  if (self === void 0) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }
  return self;
}
function _inheritsLoose2(subClass, superClass) {
  subClass.prototype = Object.create(superClass.prototype);
  subClass.prototype.constructor = subClass;
  subClass.__proto__ = superClass;
}
var gsap18;
var _win7;
var _doc8;
var _docElement3;
var _body6;
var _tempDiv3;
var _placeholderDiv;
var _coreInitted15;
var _checkPrefix;
var _toArray8;
var _supportsPassive;
var _isTouchDevice;
var _touchEventLookup;
var _isMultiTouching;
var _isAndroid;
var InertiaPlugin;
var _defaultCursor;
var _supportsPointer;
var _context5;
var _getStyleSaver8;
var _dragCount2 = 0;
var _windowExists11 = function _windowExists12() {
  return typeof window !== "undefined";
};
var _getGSAP29 = function _getGSAP30() {
  return gsap18 || _windowExists11() && (gsap18 = window.gsap) && gsap18.registerPlugin && gsap18;
};
var _isFunction9 = function _isFunction10(value) {
  return typeof value === "function";
};
var _isObject5 = function _isObject6(value) {
  return typeof value === "object";
};
var _isUndefined5 = function _isUndefined6(value) {
  return typeof value === "undefined";
};
var _emptyFunc5 = function _emptyFunc6() {
  return false;
};
var _transformProp4 = "transform";
var _transformOriginProp3 = "transformOrigin";
var _round19 = function _round20(value) {
  return Math.round(value * 1e4) / 1e4;
};
var _isArray2 = Array.isArray;
var _createElement3 = function _createElement4(type, ns) {
  var e = _doc8.createElementNS ? _doc8.createElementNS((ns || "http://www.w3.org/1999/xhtml").replace(/^https/, "http"), type) : _doc8.createElement(type);
  return e.style ? e : _doc8.createElement(type);
};
var _RAD2DEG4 = 180 / Math.PI;
var _bigNum5 = 1e20;
var _identityMatrix2 = new Matrix2D();
var _getTime3 = Date.now || function() {
  return (/* @__PURE__ */ new Date()).getTime();
};
var _renderQueue = [];
var _lookup = {};
var _lookupCount = 0;
var _clickableTagExp = /^(?:a|input|textarea|button|select)$/i;
var _lastDragTime = 0;
var _temp1 = {};
var _windowProxy = {};
var _copy3 = function _copy4(obj, factor) {
  var copy = {}, p2;
  for (p2 in obj) {
    copy[p2] = factor ? obj[p2] * factor : obj[p2];
  }
  return copy;
};
var _extend = function _extend2(obj, defaults2) {
  for (var p2 in defaults2) {
    if (!(p2 in obj)) {
      obj[p2] = defaults2[p2];
    }
  }
  return obj;
};
var _setTouchActionForAllDescendants = function _setTouchActionForAllDescendants2(elements, value) {
  var i2 = elements.length, children;
  while (i2--) {
    value ? elements[i2].style.touchAction = value : elements[i2].style.removeProperty("touch-action");
    children = elements[i2].children;
    children && children.length && _setTouchActionForAllDescendants2(children, value);
  }
};
var _renderQueueTick = function _renderQueueTick2() {
  return _renderQueue.forEach(function(func) {
    return func();
  });
};
var _addToRenderQueue = function _addToRenderQueue2(func) {
  _renderQueue.push(func);
  if (_renderQueue.length === 1) {
    gsap18.ticker.add(_renderQueueTick);
  }
};
var _renderQueueTimeout = function _renderQueueTimeout2() {
  return !_renderQueue.length && gsap18.ticker.remove(_renderQueueTick);
};
var _removeFromRenderQueue = function _removeFromRenderQueue2(func) {
  var i2 = _renderQueue.length;
  while (i2--) {
    if (_renderQueue[i2] === func) {
      _renderQueue.splice(i2, 1);
    }
  }
  gsap18.to(_renderQueueTimeout, {
    overwrite: true,
    delay: 15,
    duration: 0,
    onComplete: _renderQueueTimeout,
    data: "_draggable"
  });
};
var _setDefaults5 = function _setDefaults6(obj, defaults2) {
  for (var p2 in defaults2) {
    if (!(p2 in obj)) {
      obj[p2] = defaults2[p2];
    }
  }
  return obj;
};
var _addListener5 = function _addListener6(element, type, func, capture) {
  if (element.addEventListener) {
    var touchType = _touchEventLookup[type];
    capture = capture || (_supportsPassive ? {
      passive: false
    } : null);
    element.addEventListener(touchType || type, func, capture);
    touchType && type !== touchType && element.addEventListener(type, func, capture);
  }
};
var _removeListener5 = function _removeListener6(element, type, func, capture) {
  if (element.removeEventListener) {
    var touchType = _touchEventLookup[type];
    element.removeEventListener(touchType || type, func, capture);
    touchType && type !== touchType && element.removeEventListener(type, func, capture);
  }
};
var _preventDefault = function _preventDefault2(event) {
  event.preventDefault && event.preventDefault();
  event.preventManipulation && event.preventManipulation();
};
var _hasTouchID = function _hasTouchID2(list, ID) {
  var i2 = list.length;
  while (i2--) {
    if (list[i2].identifier === ID) {
      return true;
    }
  }
};
var _onMultiTouchDocumentEnd = function _onMultiTouchDocumentEnd2(event) {
  _isMultiTouching = event.touches && _dragCount2 < event.touches.length;
  _removeListener5(event.target, "touchend", _onMultiTouchDocumentEnd2);
};
var _onMultiTouchDocument = function _onMultiTouchDocument2(event) {
  _isMultiTouching = event.touches && _dragCount2 < event.touches.length;
  _addListener5(event.target, "touchend", _onMultiTouchDocumentEnd);
};
var _getDocScrollTop3 = function _getDocScrollTop4(doc) {
  return _win7.pageYOffset || doc.scrollTop || doc.documentElement.scrollTop || doc.body.scrollTop || 0;
};
var _getDocScrollLeft3 = function _getDocScrollLeft4(doc) {
  return _win7.pageXOffset || doc.scrollLeft || doc.documentElement.scrollLeft || doc.body.scrollLeft || 0;
};
var _addScrollListener = function _addScrollListener2(e, callback) {
  _addListener5(e, "scroll", callback);
  if (!_isRoot(e.parentNode)) {
    _addScrollListener2(e.parentNode, callback);
  }
};
var _removeScrollListener = function _removeScrollListener2(e, callback) {
  _removeListener5(e, "scroll", callback);
  if (!_isRoot(e.parentNode)) {
    _removeScrollListener2(e.parentNode, callback);
  }
};
var _isRoot = function _isRoot2(e) {
  return !!(!e || e === _docElement3 || e.nodeType === 9 || e === _doc8.body || e === _win7 || !e.nodeType || !e.parentNode);
};
var _getMaxScroll = function _getMaxScroll2(element, axis) {
  var dim = axis === "x" ? "Width" : "Height", scroll = "scroll" + dim, client = "client" + dim;
  return Math.max(0, _isRoot(element) ? Math.max(_docElement3[scroll], _body6[scroll]) - (_win7["inner" + dim] || _docElement3[client] || _body6[client]) : element[scroll] - element[client]);
};
var _recordMaxScrolls = function _recordMaxScrolls2(e, skipCurrent) {
  var x = _getMaxScroll(e, "x"), y = _getMaxScroll(e, "y");
  if (_isRoot(e)) {
    e = _windowProxy;
  } else {
    _recordMaxScrolls2(e.parentNode, skipCurrent);
  }
  e._gsMaxScrollX = x;
  e._gsMaxScrollY = y;
  if (!skipCurrent) {
    e._gsScrollX = e.scrollLeft || 0;
    e._gsScrollY = e.scrollTop || 0;
  }
};
var _setStyle = function _setStyle2(element, property, value) {
  var style = element.style;
  if (!style) {
    return;
  }
  if (_isUndefined5(style[property])) {
    property = _checkPrefix(property, element) || property;
  }
  if (value == null) {
    style.removeProperty && style.removeProperty(property.replace(/([A-Z])/g, "-$1").toLowerCase());
  } else {
    style[property] = value;
  }
};
var _getComputedStyle3 = function _getComputedStyle4(element) {
  return _win7.getComputedStyle(element instanceof Element ? element : element.host || (element.parentNode || {}).host || element);
};
var _tempRect = {};
var _parseRect = function _parseRect2(e) {
  if (e === _win7) {
    _tempRect.left = _tempRect.top = 0;
    _tempRect.width = _tempRect.right = _docElement3.clientWidth || e.innerWidth || _body6.clientWidth || 0;
    _tempRect.height = _tempRect.bottom = (e.innerHeight || 0) - 20 < _docElement3.clientHeight ? _docElement3.clientHeight : e.innerHeight || _body6.clientHeight || 0;
    return _tempRect;
  }
  var doc = e.ownerDocument || _doc8, r = !_isUndefined5(e.pageX) ? {
    left: e.pageX - _getDocScrollLeft3(doc),
    top: e.pageY - _getDocScrollTop3(doc),
    right: e.pageX - _getDocScrollLeft3(doc) + 1,
    bottom: e.pageY - _getDocScrollTop3(doc) + 1
  } : !e.nodeType && !_isUndefined5(e.left) && !_isUndefined5(e.top) ? e : _toArray8(e)[0].getBoundingClientRect();
  if (_isUndefined5(r.right) && !_isUndefined5(r.width)) {
    r.right = r.left + r.width;
    r.bottom = r.top + r.height;
  } else if (_isUndefined5(r.width)) {
    r = {
      width: r.right - r.left,
      height: r.bottom - r.top,
      right: r.right,
      left: r.left,
      bottom: r.bottom,
      top: r.top
    };
  }
  return r;
};
var _dispatchEvent = function _dispatchEvent2(target, type, callbackName) {
  var vars = target.vars, callback = vars[callbackName], listeners = target._listeners[type], result;
  if (_isFunction9(callback)) {
    result = callback.apply(vars.callbackScope || target, vars[callbackName + "Params"] || [target.pointerEvent]);
  }
  if (listeners && target.dispatchEvent(type) === false) {
    result = false;
  }
  return result;
};
var _getBounds3 = function _getBounds4(target, context3) {
  var e = _toArray8(target)[0], top, left, offset;
  if (!e.nodeType && e !== _win7) {
    if (!_isUndefined5(target.left)) {
      offset = {
        x: 0,
        y: 0
      };
      return {
        left: target.left - offset.x,
        top: target.top - offset.y,
        width: target.width,
        height: target.height
      };
    }
    left = target.min || target.minX || target.minRotation || 0;
    top = target.min || target.minY || 0;
    return {
      left,
      top,
      width: (target.max || target.maxX || target.maxRotation || 0) - left,
      height: (target.max || target.maxY || 0) - top
    };
  }
  return _getElementBounds(e, context3);
};
var _point1 = {};
var _getElementBounds = function _getElementBounds2(element, context3) {
  context3 = _toArray8(context3)[0];
  var isSVG = element.getBBox && element.ownerSVGElement, doc = element.ownerDocument || _doc8, left, right, top, bottom, matrix, p1, p2, p3, p4, bbox, width, height, cs;
  if (element === _win7) {
    top = _getDocScrollTop3(doc);
    left = _getDocScrollLeft3(doc);
    right = left + (doc.documentElement.clientWidth || element.innerWidth || doc.body.clientWidth || 0);
    bottom = top + ((element.innerHeight || 0) - 20 < doc.documentElement.clientHeight ? doc.documentElement.clientHeight : element.innerHeight || doc.body.clientHeight || 0);
  } else if (context3 === _win7 || _isUndefined5(context3)) {
    return element.getBoundingClientRect();
  } else {
    left = top = 0;
    if (isSVG) {
      bbox = element.getBBox();
      width = bbox.width;
      height = bbox.height;
    } else {
      if (element.viewBox && (bbox = element.viewBox.baseVal)) {
        left = bbox.x || 0;
        top = bbox.y || 0;
        width = bbox.width;
        height = bbox.height;
      }
      if (!width) {
        cs = _getComputedStyle3(element);
        bbox = cs.boxSizing === "border-box";
        width = (parseFloat(cs.width) || element.clientWidth || 0) + (bbox ? 0 : parseFloat(cs.borderLeftWidth) + parseFloat(cs.borderRightWidth));
        height = (parseFloat(cs.height) || element.clientHeight || 0) + (bbox ? 0 : parseFloat(cs.borderTopWidth) + parseFloat(cs.borderBottomWidth));
      }
    }
    right = width;
    bottom = height;
  }
  if (element === context3) {
    return {
      left,
      top,
      width: right - left,
      height: bottom - top
    };
  }
  matrix = getGlobalMatrix(context3, true).multiply(getGlobalMatrix(element));
  p1 = matrix.apply({
    x: left,
    y: top
  });
  p2 = matrix.apply({
    x: right,
    y: top
  });
  p3 = matrix.apply({
    x: right,
    y: bottom
  });
  p4 = matrix.apply({
    x: left,
    y: bottom
  });
  left = Math.min(p1.x, p2.x, p3.x, p4.x);
  top = Math.min(p1.y, p2.y, p3.y, p4.y);
  return {
    left,
    top,
    width: Math.max(p1.x, p2.x, p3.x, p4.x) - left,
    height: Math.max(p1.y, p2.y, p3.y, p4.y) - top
  };
};
var _parseInertia = function _parseInertia2(draggable, snap3, max, min, factor, forceZeroVelocity) {
  var vars = {}, a, i2, l;
  if (snap3) {
    if (factor !== 1 && snap3 instanceof Array) {
      vars.end = a = [];
      l = snap3.length;
      if (_isObject5(snap3[0])) {
        for (i2 = 0; i2 < l; i2++) {
          a[i2] = _copy3(snap3[i2], factor);
        }
      } else {
        for (i2 = 0; i2 < l; i2++) {
          a[i2] = snap3[i2] * factor;
        }
      }
      max += 1.1;
      min -= 1.1;
    } else if (_isFunction9(snap3)) {
      vars.end = function(value) {
        var result = snap3.call(draggable, value), copy, p2;
        if (factor !== 1) {
          if (_isObject5(result)) {
            copy = {};
            for (p2 in result) {
              copy[p2] = result[p2] * factor;
            }
            result = copy;
          } else {
            result *= factor;
          }
        }
        return result;
      };
    } else {
      vars.end = snap3;
    }
  }
  if (max || max === 0) {
    vars.max = max;
  }
  if (min || min === 0) {
    vars.min = min;
  }
  if (forceZeroVelocity) {
    vars.velocity = 0;
  }
  return vars;
};
var _isClickable = function _isClickable2(element) {
  var data;
  return !element || !element.getAttribute || element === _body6 ? false : (data = element.getAttribute("data-clickable")) === "true" || data !== "false" && (_clickableTagExp.test(element.nodeName + "") || element.getAttribute("contentEditable") === "true") ? true : _isClickable2(element.parentNode);
};
var _setSelectable = function _setSelectable2(elements, selectable) {
  var i2 = elements.length, e;
  while (i2--) {
    e = elements[i2];
    e.ondragstart = e.onselectstart = selectable ? null : _emptyFunc5;
    gsap18.set(e, {
      lazy: true,
      userSelect: selectable ? "text" : "none"
    });
  }
};
var _isFixed3 = function _isFixed4(element) {
  if (_getComputedStyle3(element).position === "fixed") {
    return true;
  }
  element = element.parentNode;
  if (element && element.nodeType === 1) {
    return _isFixed4(element);
  }
};
var _supports3D2;
var _addPaddingBR;
var ScrollProxy = function ScrollProxy2(element, vars) {
  element = gsap18.utils.toArray(element)[0];
  vars = vars || {};
  var content = document.createElement("div"), style = content.style, node = element.firstChild, offsetTop = 0, offsetLeft = 0, prevTop = element.scrollTop, prevLeft = element.scrollLeft, scrollWidth = element.scrollWidth, scrollHeight = element.scrollHeight, extraPadRight = 0, maxLeft = 0, maxTop = 0, elementWidth, elementHeight, contentHeight, nextNode, transformStart, transformEnd;
  if (_supports3D2 && vars.force3D !== false) {
    transformStart = "translate3d(";
    transformEnd = "px,0px)";
  } else if (_transformProp4) {
    transformStart = "translate(";
    transformEnd = "px)";
  }
  this.scrollTop = function(value, force) {
    if (!arguments.length) {
      return -this.top();
    }
    this.top(-value, force);
  };
  this.scrollLeft = function(value, force) {
    if (!arguments.length) {
      return -this.left();
    }
    this.left(-value, force);
  };
  this.left = function(value, force) {
    if (!arguments.length) {
      return -(element.scrollLeft + offsetLeft);
    }
    var dif = element.scrollLeft - prevLeft, oldOffset = offsetLeft;
    if ((dif > 2 || dif < -2) && !force) {
      prevLeft = element.scrollLeft;
      gsap18.killTweensOf(this, {
        left: 1,
        scrollLeft: 1
      });
      this.left(-prevLeft);
      if (vars.onKill) {
        vars.onKill();
      }
      return;
    }
    value = -value;
    if (value < 0) {
      offsetLeft = value - 0.5 | 0;
      value = 0;
    } else if (value > maxLeft) {
      offsetLeft = value - maxLeft | 0;
      value = maxLeft;
    } else {
      offsetLeft = 0;
    }
    if (offsetLeft || oldOffset) {
      if (!this._skip) {
        style[_transformProp4] = transformStart + -offsetLeft + "px," + -offsetTop + transformEnd;
      }
      if (offsetLeft + extraPadRight >= 0) {
        style.paddingRight = offsetLeft + extraPadRight + "px";
      }
    }
    element.scrollLeft = value | 0;
    prevLeft = element.scrollLeft;
  };
  this.top = function(value, force) {
    if (!arguments.length) {
      return -(element.scrollTop + offsetTop);
    }
    var dif = element.scrollTop - prevTop, oldOffset = offsetTop;
    if ((dif > 2 || dif < -2) && !force) {
      prevTop = element.scrollTop;
      gsap18.killTweensOf(this, {
        top: 1,
        scrollTop: 1
      });
      this.top(-prevTop);
      if (vars.onKill) {
        vars.onKill();
      }
      return;
    }
    value = -value;
    if (value < 0) {
      offsetTop = value - 0.5 | 0;
      value = 0;
    } else if (value > maxTop) {
      offsetTop = value - maxTop | 0;
      value = maxTop;
    } else {
      offsetTop = 0;
    }
    if (offsetTop || oldOffset) {
      if (!this._skip) {
        style[_transformProp4] = transformStart + -offsetLeft + "px," + -offsetTop + transformEnd;
      }
    }
    element.scrollTop = value | 0;
    prevTop = element.scrollTop;
  };
  this.maxScrollTop = function() {
    return maxTop;
  };
  this.maxScrollLeft = function() {
    return maxLeft;
  };
  this.disable = function() {
    node = content.firstChild;
    while (node) {
      nextNode = node.nextSibling;
      element.appendChild(node);
      node = nextNode;
    }
    if (element === content.parentNode) {
      element.removeChild(content);
    }
  };
  this.enable = function() {
    node = element.firstChild;
    if (node === content) {
      return;
    }
    while (node) {
      nextNode = node.nextSibling;
      content.appendChild(node);
      node = nextNode;
    }
    element.appendChild(content);
    this.calibrate();
  };
  this.calibrate = function(force) {
    var widthMatches = element.clientWidth === elementWidth, cs, x, y;
    prevTop = element.scrollTop;
    prevLeft = element.scrollLeft;
    if (widthMatches && element.clientHeight === elementHeight && content.offsetHeight === contentHeight && scrollWidth === element.scrollWidth && scrollHeight === element.scrollHeight && !force) {
      return;
    }
    if (offsetTop || offsetLeft) {
      x = this.left();
      y = this.top();
      this.left(-element.scrollLeft);
      this.top(-element.scrollTop);
    }
    cs = _getComputedStyle3(element);
    if (!widthMatches || force) {
      style.display = "block";
      style.width = "auto";
      style.paddingRight = "0px";
      extraPadRight = Math.max(0, element.scrollWidth - element.clientWidth);
      if (extraPadRight) {
        extraPadRight += parseFloat(cs.paddingLeft) + (_addPaddingBR ? parseFloat(cs.paddingRight) : 0);
      }
    }
    style.display = "inline-block";
    style.position = "relative";
    style.overflow = "visible";
    style.verticalAlign = "top";
    style.boxSizing = "content-box";
    style.width = "100%";
    style.paddingRight = extraPadRight + "px";
    if (_addPaddingBR) {
      style.paddingBottom = cs.paddingBottom;
    }
    elementWidth = element.clientWidth;
    elementHeight = element.clientHeight;
    scrollWidth = element.scrollWidth;
    scrollHeight = element.scrollHeight;
    maxLeft = element.scrollWidth - elementWidth;
    maxTop = element.scrollHeight - elementHeight;
    contentHeight = content.offsetHeight;
    style.display = "block";
    if (x || y) {
      this.left(x);
      this.top(y);
    }
  };
  this.content = content;
  this.element = element;
  this._skip = false;
  this.enable();
};
var _initCore25 = function _initCore26(required) {
  if (_windowExists11() && document.body) {
    var nav = window && window.navigator;
    _win7 = window;
    _doc8 = document;
    _docElement3 = _doc8.documentElement;
    _body6 = _doc8.body;
    _tempDiv3 = _createElement3("div");
    _supportsPointer = !!window.PointerEvent;
    _placeholderDiv = _createElement3("div");
    _placeholderDiv.style.cssText = "visibility:hidden;height:1px;top:-1px;pointer-events:none;position:relative;clear:both;cursor:grab";
    _defaultCursor = _placeholderDiv.style.cursor === "grab" ? "grab" : "move";
    _isAndroid = nav && nav.userAgent.toLowerCase().indexOf("android") !== -1;
    _isTouchDevice = "ontouchstart" in _docElement3 && "orientation" in _win7 || nav && (nav.MaxTouchPoints > 0 || nav.msMaxTouchPoints > 0);
    _addPaddingBR = (function() {
      var div = _createElement3("div"), child = _createElement3("div"), childStyle = child.style, parent = _body6, val;
      childStyle.display = "inline-block";
      childStyle.position = "relative";
      div.style.cssText = "width:90px;height:40px;padding:10px;overflow:auto;visibility:hidden";
      div.appendChild(child);
      parent.appendChild(div);
      val = child.offsetHeight + 18 > div.scrollHeight;
      parent.removeChild(div);
      return val;
    })();
    _touchEventLookup = (function(types) {
      var standard = types.split(","), converted = ("onpointerdown" in _tempDiv3 ? "pointerdown,pointermove,pointerup,pointercancel" : "onmspointerdown" in _tempDiv3 ? "MSPointerDown,MSPointerMove,MSPointerUp,MSPointerCancel" : types).split(","), obj = {}, i2 = 4;
      while (--i2 > -1) {
        obj[standard[i2]] = converted[i2];
        obj[converted[i2]] = standard[i2];
      }
      try {
        _docElement3.addEventListener("test", null, Object.defineProperty({}, "passive", {
          get: function get() {
            _supportsPassive = 1;
          }
        }));
      } catch (e) {
      }
      return obj;
    })("touchstart,touchmove,touchend,touchcancel");
    _addListener5(_doc8, "touchcancel", _emptyFunc5);
    _addListener5(_win7, "touchmove", _emptyFunc5);
    _body6 && _body6.addEventListener("touchstart", _emptyFunc5);
    _addListener5(_doc8, "contextmenu", function() {
      for (var p2 in _lookup) {
        if (_lookup[p2].isPressed) {
          _lookup[p2].endDrag();
        }
      }
    });
    gsap18 = _coreInitted15 = _getGSAP29();
  }
  if (gsap18) {
    InertiaPlugin = gsap18.plugins.inertia;
    _context5 = gsap18.core.context || function() {
    };
    _checkPrefix = gsap18.utils.checkPrefix;
    _transformProp4 = _checkPrefix(_transformProp4);
    _transformOriginProp3 = _checkPrefix(_transformOriginProp3);
    _toArray8 = gsap18.utils.toArray;
    _getStyleSaver8 = gsap18.core.getStyleSaver;
    _supports3D2 = !!_checkPrefix("perspective");
  } else if (required) {
    console.warn("Please gsap.registerPlugin(Draggable)");
  }
};
var EventDispatcher = /* @__PURE__ */ (function() {
  function EventDispatcher2(target) {
    this._listeners = {};
    this.target = target || this;
  }
  var _proto = EventDispatcher2.prototype;
  _proto.addEventListener = function addEventListener2(type, callback) {
    var list = this._listeners[type] || (this._listeners[type] = []);
    if (!~list.indexOf(callback)) {
      list.push(callback);
    }
  };
  _proto.removeEventListener = function removeEventListener2(type, callback) {
    var list = this._listeners[type], i2 = list && list.indexOf(callback);
    i2 >= 0 && list.splice(i2, 1);
  };
  _proto.dispatchEvent = function dispatchEvent(type) {
    var _this = this;
    var result;
    (this._listeners[type] || []).forEach(function(callback) {
      return callback.call(_this, {
        type,
        target: _this.target
      }) === false && (result = false);
    });
    return result;
  };
  return EventDispatcher2;
})();
var Draggable = /* @__PURE__ */ (function(_EventDispatcher) {
  _inheritsLoose2(Draggable2, _EventDispatcher);
  function Draggable2(target, vars) {
    var _this2;
    _this2 = _EventDispatcher.call(this) || this;
    _coreInitted15 || _initCore25(1);
    target = _toArray8(target)[0];
    _this2.styles = _getStyleSaver8 && _getStyleSaver8(target, "transform,left,top");
    if (!InertiaPlugin) {
      InertiaPlugin = gsap18.plugins.inertia;
    }
    _this2.vars = vars = _copy3(vars || {});
    _this2.target = target;
    _this2.x = _this2.y = _this2.rotation = 0;
    _this2.dragResistance = parseFloat(vars.dragResistance) || 0;
    _this2.edgeResistance = isNaN(vars.edgeResistance) ? 1 : parseFloat(vars.edgeResistance) || 0;
    _this2.lockAxis = vars.lockAxis;
    _this2.autoScroll = vars.autoScroll || 0;
    _this2.lockedAxis = null;
    _this2.allowEventDefault = !!vars.allowEventDefault;
    gsap18.getProperty(target, "x");
    var type = (vars.type || "x,y").toLowerCase(), xyMode = ~type.indexOf("x") || ~type.indexOf("y"), rotationMode = type.indexOf("rotation") !== -1, xProp = rotationMode ? "rotation" : xyMode ? "x" : "left", yProp = xyMode ? "y" : "top", allowX = !!(~type.indexOf("x") || ~type.indexOf("left") || type === "scroll"), allowY = !!(~type.indexOf("y") || ~type.indexOf("top") || type === "scroll"), minimumMovement = vars.minimumMovement || 2, self = _assertThisInitialized2(_this2), triggers = _toArray8(vars.trigger || vars.handle || target), killProps = {}, dragEndTime = 0, checkAutoScrollBounds = false, autoScrollMarginTop = vars.autoScrollMarginTop || 40, autoScrollMarginRight = vars.autoScrollMarginRight || 40, autoScrollMarginBottom = vars.autoScrollMarginBottom || 40, autoScrollMarginLeft = vars.autoScrollMarginLeft || 40, isClickable = vars.clickableTest || _isClickable, clickTime = 0, gsCache = target._gsap || gsap18.core.getCache(target), isFixed = _isFixed3(target), getPropAsNum = function getPropAsNum2(property, unit) {
      return parseFloat(gsCache.get(target, property, unit));
    }, ownerDoc = target.ownerDocument || _doc8, enabled, scrollProxy, startPointerX, startPointerY, startElementX, startElementY, hasBounds, hasDragCallback, hasMoveCallback, maxX, minX, maxY, minY, touch, touchID, rotationOrigin, dirty, old, snapX, snapY, snapXY, isClicking, touchEventTarget, matrix, interrupted, allowNativeTouchScrolling, touchDragAxis, isDispatching, clickDispatch, trustedClickDispatch, isPreventingDefault, innerMatrix, dragged, onContextMenu = function onContextMenu2(e) {
      _preventDefault(e);
      e.stopImmediatePropagation && e.stopImmediatePropagation();
      return false;
    }, render14 = function render15(suppressEvents) {
      if (self.autoScroll && self.isDragging && (checkAutoScrollBounds || dirty)) {
        var e = target, autoScrollFactor = self.autoScroll * 15, parent, isRoot, rect, pointerX, pointerY, changeX, changeY, gap;
        checkAutoScrollBounds = false;
        _windowProxy.scrollTop = _win7.pageYOffset != null ? _win7.pageYOffset : ownerDoc.documentElement.scrollTop != null ? ownerDoc.documentElement.scrollTop : ownerDoc.body.scrollTop;
        _windowProxy.scrollLeft = _win7.pageXOffset != null ? _win7.pageXOffset : ownerDoc.documentElement.scrollLeft != null ? ownerDoc.documentElement.scrollLeft : ownerDoc.body.scrollLeft;
        pointerX = self.pointerX - _windowProxy.scrollLeft;
        pointerY = self.pointerY - _windowProxy.scrollTop;
        while (e && !isRoot) {
          isRoot = _isRoot(e.parentNode);
          parent = isRoot ? _windowProxy : e.parentNode;
          rect = isRoot ? {
            bottom: Math.max(_docElement3.clientHeight, _win7.innerHeight || 0),
            right: Math.max(_docElement3.clientWidth, _win7.innerWidth || 0),
            left: 0,
            top: 0
          } : parent.getBoundingClientRect();
          changeX = changeY = 0;
          if (allowY) {
            gap = parent._gsMaxScrollY - parent.scrollTop;
            if (gap < 0) {
              changeY = gap;
            } else if (pointerY > rect.bottom - autoScrollMarginBottom && gap) {
              checkAutoScrollBounds = true;
              changeY = Math.min(gap, autoScrollFactor * (1 - Math.max(0, rect.bottom - pointerY) / autoScrollMarginBottom) | 0);
            } else if (pointerY < rect.top + autoScrollMarginTop && parent.scrollTop) {
              checkAutoScrollBounds = true;
              changeY = -Math.min(parent.scrollTop, autoScrollFactor * (1 - Math.max(0, pointerY - rect.top) / autoScrollMarginTop) | 0);
            }
            if (changeY) {
              parent.scrollTop += changeY;
            }
          }
          if (allowX) {
            gap = parent._gsMaxScrollX - parent.scrollLeft;
            if (gap < 0) {
              changeX = gap;
            } else if (pointerX > rect.right - autoScrollMarginRight && gap) {
              checkAutoScrollBounds = true;
              changeX = Math.min(gap, autoScrollFactor * (1 - Math.max(0, rect.right - pointerX) / autoScrollMarginRight) | 0);
            } else if (pointerX < rect.left + autoScrollMarginLeft && parent.scrollLeft) {
              checkAutoScrollBounds = true;
              changeX = -Math.min(parent.scrollLeft, autoScrollFactor * (1 - Math.max(0, pointerX - rect.left) / autoScrollMarginLeft) | 0);
            }
            if (changeX) {
              parent.scrollLeft += changeX;
            }
          }
          if (isRoot && (changeX || changeY)) {
            _win7.scrollTo(parent.scrollLeft, parent.scrollTop);
            setPointerPosition(self.pointerX + changeX, self.pointerY + changeY);
          }
          e = parent;
        }
      }
      if (dirty) {
        var x = self.x, y = self.y;
        if (rotationMode) {
          self.deltaX = x - parseFloat(gsCache.rotation);
          self.rotation = x;
          gsCache.rotation = x + "deg";
          gsCache.renderTransform(1, gsCache);
        } else {
          if (scrollProxy) {
            if (allowY) {
              self.deltaY = y - scrollProxy.top();
              scrollProxy.top(y);
            }
            if (allowX) {
              self.deltaX = x - scrollProxy.left();
              scrollProxy.left(x);
            }
          } else if (xyMode) {
            if (allowY) {
              self.deltaY = y - parseFloat(gsCache.y);
              gsCache.y = y + "px";
            }
            if (allowX) {
              self.deltaX = x - parseFloat(gsCache.x);
              gsCache.x = x + "px";
            }
            gsCache.renderTransform(1, gsCache);
          } else {
            if (allowY) {
              self.deltaY = y - parseFloat(target.style.top || 0);
              target.style.top = y + "px";
            }
            if (allowX) {
              self.deltaX = x - parseFloat(target.style.left || 0);
              target.style.left = x + "px";
            }
          }
        }
        if (hasDragCallback && !suppressEvents && !isDispatching) {
          isDispatching = true;
          if (_dispatchEvent(self, "drag", "onDrag") === false) {
            if (allowX) {
              self.x -= self.deltaX;
            }
            if (allowY) {
              self.y -= self.deltaY;
            }
            render15(true);
          }
          isDispatching = false;
        }
      }
      dirty = false;
    }, syncXY = function syncXY2(skipOnUpdate, skipSnap) {
      var x = self.x, y = self.y, snappedValue, cs;
      if (!target._gsap) {
        gsCache = gsap18.core.getCache(target);
      }
      gsCache.uncache && gsap18.getProperty(target, "x");
      if (xyMode) {
        self.x = parseFloat(gsCache.x);
        self.y = parseFloat(gsCache.y);
      } else if (rotationMode) {
        self.x = self.rotation = _round19(parseFloat(gsCache.rotation));
      } else if (scrollProxy) {
        self.y = scrollProxy.top();
        self.x = scrollProxy.left();
      } else {
        self.y = parseFloat(target.style.top || (cs = _getComputedStyle3(target)) && cs.top) || 0;
        self.x = parseFloat(target.style.left || (cs || {}).left) || 0;
      }
      if ((snapX || snapY || snapXY) && !skipSnap && (self.isDragging || self.isThrowing)) {
        if (snapXY) {
          _temp1.x = self.x;
          _temp1.y = self.y;
          snappedValue = snapXY(_temp1);
          if (snappedValue.x !== self.x) {
            self.x = snappedValue.x;
            dirty = true;
          }
          if (snappedValue.y !== self.y) {
            self.y = snappedValue.y;
            dirty = true;
          }
        }
        if (snapX) {
          snappedValue = snapX(self.x);
          if (snappedValue !== self.x) {
            self.x = snappedValue;
            if (rotationMode) {
              self.rotation = snappedValue;
            }
            dirty = true;
          }
        }
        if (snapY) {
          snappedValue = snapY(self.y);
          if (snappedValue !== self.y) {
            self.y = snappedValue;
          }
          dirty = true;
        }
      }
      dirty && render14(true);
      if (!skipOnUpdate) {
        self.deltaX = self.x - x;
        self.deltaY = self.y - y;
        _dispatchEvent(self, "throwupdate", "onThrowUpdate");
      }
    }, buildSnapFunc = function buildSnapFunc2(snap3, min, max, factor) {
      if (min == null) {
        min = -_bigNum5;
      }
      if (max == null) {
        max = _bigNum5;
      }
      if (_isFunction9(snap3)) {
        return function(n) {
          var edgeTolerance = !self.isPressed ? 1 : 1 - self.edgeResistance;
          return snap3.call(self, (n > max ? max + (n - max) * edgeTolerance : n < min ? min + (n - min) * edgeTolerance : n) * factor) * factor;
        };
      }
      if (_isArray2(snap3)) {
        return function(n) {
          var i2 = snap3.length, closest = 0, absDif = _bigNum5, val, dif;
          while (--i2 > -1) {
            val = snap3[i2];
            dif = val - n;
            if (dif < 0) {
              dif = -dif;
            }
            if (dif < absDif && val >= min && val <= max) {
              closest = i2;
              absDif = dif;
            }
          }
          return snap3[closest];
        };
      }
      return isNaN(snap3) ? function(n) {
        return n;
      } : function() {
        return snap3 * factor;
      };
    }, buildPointSnapFunc = function buildPointSnapFunc2(snap3, minX2, maxX2, minY2, maxY2, radius, factor) {
      radius = radius && radius < _bigNum5 ? radius * radius : _bigNum5;
      if (_isFunction9(snap3)) {
        return function(point) {
          var edgeTolerance = !self.isPressed ? 1 : 1 - self.edgeResistance, x = point.x, y = point.y, result, dx, dy;
          point.x = x = x > maxX2 ? maxX2 + (x - maxX2) * edgeTolerance : x < minX2 ? minX2 + (x - minX2) * edgeTolerance : x;
          point.y = y = y > maxY2 ? maxY2 + (y - maxY2) * edgeTolerance : y < minY2 ? minY2 + (y - minY2) * edgeTolerance : y;
          result = snap3.call(self, point);
          if (result !== point) {
            point.x = result.x;
            point.y = result.y;
          }
          if (factor !== 1) {
            point.x *= factor;
            point.y *= factor;
          }
          if (radius < _bigNum5) {
            dx = point.x - x;
            dy = point.y - y;
            if (dx * dx + dy * dy > radius) {
              point.x = x;
              point.y = y;
            }
          }
          return point;
        };
      }
      if (_isArray2(snap3)) {
        return function(p2) {
          var i2 = snap3.length, closest = 0, minDist = _bigNum5, x, y, point, dist;
          while (--i2 > -1) {
            point = snap3[i2];
            x = point.x - p2.x;
            y = point.y - p2.y;
            dist = x * x + y * y;
            if (dist < minDist) {
              closest = i2;
              minDist = dist;
            }
          }
          return minDist <= radius ? snap3[closest] : p2;
        };
      }
      return function(n) {
        return n;
      };
    }, calculateBounds = function calculateBounds2() {
      var bounds, targetBounds, snap3, snapIsRaw;
      hasBounds = false;
      if (scrollProxy) {
        scrollProxy.calibrate();
        self.minX = minX = -scrollProxy.maxScrollLeft();
        self.minY = minY = -scrollProxy.maxScrollTop();
        self.maxX = maxX = self.maxY = maxY = 0;
        hasBounds = true;
      } else if (!!vars.bounds) {
        bounds = _getBounds3(vars.bounds, target.parentNode);
        if (rotationMode) {
          self.minX = minX = bounds.left;
          self.maxX = maxX = bounds.left + bounds.width;
          self.minY = minY = self.maxY = maxY = 0;
        } else if (!_isUndefined5(vars.bounds.maxX) || !_isUndefined5(vars.bounds.maxY)) {
          bounds = vars.bounds;
          self.minX = minX = bounds.minX;
          self.minY = minY = bounds.minY;
          self.maxX = maxX = bounds.maxX;
          self.maxY = maxY = bounds.maxY;
        } else {
          targetBounds = _getBounds3(target, target.parentNode);
          self.minX = minX = Math.round(getPropAsNum(xProp, "px") + bounds.left - targetBounds.left);
          self.minY = minY = Math.round(getPropAsNum(yProp, "px") + bounds.top - targetBounds.top);
          self.maxX = maxX = Math.round(minX + (bounds.width - targetBounds.width));
          self.maxY = maxY = Math.round(minY + (bounds.height - targetBounds.height));
        }
        if (minX > maxX) {
          self.minX = maxX;
          self.maxX = maxX = minX;
          minX = self.minX;
        }
        if (minY > maxY) {
          self.minY = maxY;
          self.maxY = maxY = minY;
          minY = self.minY;
        }
        if (rotationMode) {
          self.minRotation = minX;
          self.maxRotation = maxX;
        }
        hasBounds = true;
      }
      if (vars.liveSnap) {
        snap3 = vars.liveSnap === true ? vars.snap || {} : vars.liveSnap;
        snapIsRaw = _isArray2(snap3) || _isFunction9(snap3);
        if (rotationMode) {
          snapX = buildSnapFunc(snapIsRaw ? snap3 : snap3.rotation, minX, maxX, 1);
          snapY = null;
        } else {
          if (snap3.points) {
            snapXY = buildPointSnapFunc(snapIsRaw ? snap3 : snap3.points, minX, maxX, minY, maxY, snap3.radius, scrollProxy ? -1 : 1);
          } else {
            if (allowX) {
              snapX = buildSnapFunc(snapIsRaw ? snap3 : snap3.x || snap3.left || snap3.scrollLeft, minX, maxX, scrollProxy ? -1 : 1);
            }
            if (allowY) {
              snapY = buildSnapFunc(snapIsRaw ? snap3 : snap3.y || snap3.top || snap3.scrollTop, minY, maxY, scrollProxy ? -1 : 1);
            }
          }
        }
      }
    }, onThrowComplete = function onThrowComplete2() {
      self.isThrowing = false;
      _dispatchEvent(self, "throwcomplete", "onThrowComplete");
    }, onThrowInterrupt = function onThrowInterrupt2() {
      self.isThrowing = false;
    }, animate = function animate2(inertia, forceZeroVelocity) {
      var snap3, snapIsRaw, tween, overshootTolerance;
      if (inertia && InertiaPlugin) {
        if (inertia === true) {
          snap3 = vars.snap || vars.liveSnap || {};
          snapIsRaw = _isArray2(snap3) || _isFunction9(snap3);
          inertia = {
            resistance: (vars.throwResistance || vars.resistance || 1e3) / (rotationMode ? 10 : 1)
          };
          if (rotationMode) {
            inertia.rotation = _parseInertia(self, snapIsRaw ? snap3 : snap3.rotation, maxX, minX, 1, forceZeroVelocity);
          } else {
            if (allowX) {
              inertia[xProp] = _parseInertia(self, snapIsRaw ? snap3 : snap3.points || snap3.x || snap3.left, maxX, minX, scrollProxy ? -1 : 1, forceZeroVelocity || self.lockedAxis === "x");
            }
            if (allowY) {
              inertia[yProp] = _parseInertia(self, snapIsRaw ? snap3 : snap3.points || snap3.y || snap3.top, maxY, minY, scrollProxy ? -1 : 1, forceZeroVelocity || self.lockedAxis === "y");
            }
            if (snap3.points || _isArray2(snap3) && _isObject5(snap3[0])) {
              inertia.linkedProps = xProp + "," + yProp;
              inertia.radius = snap3.radius;
            }
          }
        }
        self.isThrowing = true;
        overshootTolerance = !isNaN(vars.overshootTolerance) ? vars.overshootTolerance : vars.edgeResistance === 1 ? 0 : 1 - self.edgeResistance + 0.2;
        if (!inertia.duration) {
          inertia.duration = {
            max: Math.max(vars.minDuration || 0, "maxDuration" in vars ? vars.maxDuration : 2),
            min: !isNaN(vars.minDuration) ? vars.minDuration : overshootTolerance === 0 || _isObject5(inertia) && inertia.resistance > 1e3 ? 0 : 0.5,
            overshoot: overshootTolerance
          };
        }
        self.tween = tween = gsap18.to(scrollProxy || target, {
          inertia,
          data: "_draggable",
          inherit: false,
          onComplete: onThrowComplete,
          onInterrupt: onThrowInterrupt,
          onUpdate: vars.fastMode ? _dispatchEvent : syncXY,
          onUpdateParams: vars.fastMode ? [self, "onthrowupdate", "onThrowUpdate"] : snap3 && snap3.radius ? [false, true] : []
        });
        if (!vars.fastMode) {
          if (scrollProxy) {
            scrollProxy._skip = true;
          }
          tween.render(1e9, true, true);
          syncXY(true, true);
          self.endX = self.x;
          self.endY = self.y;
          if (rotationMode) {
            self.endRotation = self.x;
          }
          tween.play(0);
          syncXY(true, true);
          if (scrollProxy) {
            scrollProxy._skip = false;
          }
        }
      } else if (hasBounds) {
        self.applyBounds();
      }
    }, updateMatrix = function updateMatrix2(shiftStart) {
      var start = matrix, p2;
      matrix = getGlobalMatrix(target.parentNode, true);
      if (shiftStart && self.isPressed && !matrix.equals(start || new Matrix2D())) {
        p2 = start.inverse().apply({
          x: startPointerX,
          y: startPointerY
        });
        matrix.apply(p2, p2);
        startPointerX = p2.x;
        startPointerY = p2.y;
      }
      if (matrix.equals(_identityMatrix2)) {
        matrix = null;
      }
    }, recordStartPositions = function recordStartPositions2() {
      var edgeTolerance = 1 - self.edgeResistance, offsetX = isFixed ? _getDocScrollLeft3(ownerDoc) : 0, offsetY = isFixed ? _getDocScrollTop3(ownerDoc) : 0, parsedOrigin, x, y;
      if (xyMode) {
        gsCache.x = getPropAsNum(xProp, "px") + "px";
        gsCache.y = getPropAsNum(yProp, "px") + "px";
        gsCache.renderTransform();
      }
      updateMatrix(false);
      _point1.x = self.pointerX - offsetX;
      _point1.y = self.pointerY - offsetY;
      matrix && matrix.apply(_point1, _point1);
      startPointerX = _point1.x;
      startPointerY = _point1.y;
      if (dirty) {
        setPointerPosition(self.pointerX, self.pointerY);
        render14(true);
      }
      innerMatrix = getGlobalMatrix(target);
      if (scrollProxy) {
        calculateBounds();
        startElementY = scrollProxy.top();
        startElementX = scrollProxy.left();
      } else {
        if (isTweening2()) {
          syncXY(true, true);
          calculateBounds();
        } else {
          self.applyBounds();
        }
        if (rotationMode) {
          parsedOrigin = target.ownerSVGElement ? [gsCache.xOrigin - target.getBBox().x, gsCache.yOrigin - target.getBBox().y] : (_getComputedStyle3(target)[_transformOriginProp3] || "0 0").split(" ");
          rotationOrigin = self.rotationOrigin = getGlobalMatrix(target).apply({
            x: parseFloat(parsedOrigin[0]) || 0,
            y: parseFloat(parsedOrigin[1]) || 0
          });
          syncXY(true, true);
          x = self.pointerX - rotationOrigin.x - offsetX;
          y = rotationOrigin.y - self.pointerY + offsetY;
          startElementX = self.x;
          startElementY = self.y = Math.atan2(y, x) * _RAD2DEG4;
        } else {
          startElementY = getPropAsNum(yProp, "px");
          startElementX = getPropAsNum(xProp, "px");
        }
      }
      if (hasBounds && edgeTolerance) {
        if (startElementX > maxX) {
          startElementX = maxX + (startElementX - maxX) / edgeTolerance;
        } else if (startElementX < minX) {
          startElementX = minX - (minX - startElementX) / edgeTolerance;
        }
        if (!rotationMode) {
          if (startElementY > maxY) {
            startElementY = maxY + (startElementY - maxY) / edgeTolerance;
          } else if (startElementY < minY) {
            startElementY = minY - (minY - startElementY) / edgeTolerance;
          }
        }
      }
      self.startX = startElementX = _round19(startElementX);
      self.startY = startElementY = _round19(startElementY);
    }, isTweening2 = function isTweening3() {
      return self.tween && self.tween.isActive();
    }, removePlaceholder = function removePlaceholder2() {
      if (_placeholderDiv.parentNode && !isTweening2() && !self.isDragging) {
        _placeholderDiv.parentNode.removeChild(_placeholderDiv);
      }
    }, onPress = function onPress2(e, force) {
      var i2;
      if (!enabled || self.isPressed || !e || (e.type === "mousedown" || e.type === "pointerdown") && !force && _getTime3() - clickTime < 30 && _touchEventLookup[self.pointerEvent.type]) {
        isPreventingDefault && e && enabled && _preventDefault(e);
        return;
      }
      interrupted = isTweening2();
      dragged = false;
      self.pointerEvent = e;
      if (_touchEventLookup[e.type]) {
        touchEventTarget = ~e.type.indexOf("touch") ? e.currentTarget || e.target : ownerDoc;
        _addListener5(touchEventTarget, "touchend", onRelease);
        _addListener5(touchEventTarget, "touchmove", onMove);
        _addListener5(touchEventTarget, "touchcancel", onRelease);
        _addListener5(ownerDoc, "touchstart", _onMultiTouchDocument);
      } else {
        touchEventTarget = null;
        _addListener5(ownerDoc, "mousemove", onMove);
      }
      touchDragAxis = null;
      if (!_supportsPointer || !touchEventTarget) {
        _addListener5(ownerDoc, "mouseup", onRelease);
        e && e.target && _addListener5(e.target, "mouseup", onRelease);
      }
      isClicking = isClickable.call(self, e.target) && vars.dragClickables === false && !force;
      if (isClicking) {
        _addListener5(e.target, "change", onRelease);
        _dispatchEvent(self, "pressInit", "onPressInit");
        _dispatchEvent(self, "press", "onPress");
        _setSelectable(triggers, true);
        isPreventingDefault = false;
        return;
      }
      allowNativeTouchScrolling = !touchEventTarget || allowX === allowY || self.vars.allowNativeTouchScrolling === false || self.vars.allowContextMenu && e && (e.ctrlKey || e.which > 2) ? false : allowX ? "y" : "x";
      isPreventingDefault = !allowNativeTouchScrolling && !self.allowEventDefault;
      if (isPreventingDefault) {
        _preventDefault(e);
        _addListener5(_win7, "touchforcechange", _preventDefault);
      }
      if (e.changedTouches) {
        e = touch = e.changedTouches[0];
        touchID = e.identifier;
      } else if (e.pointerId) {
        touchID = e.pointerId;
      } else {
        touch = touchID = null;
      }
      _dragCount2++;
      _addToRenderQueue(render14);
      startPointerY = self.pointerY = e.pageY;
      startPointerX = self.pointerX = e.pageX;
      _dispatchEvent(self, "pressInit", "onPressInit");
      if (allowNativeTouchScrolling || self.autoScroll) {
        _recordMaxScrolls(target.parentNode);
      }
      if (target.parentNode && self.autoScroll && !scrollProxy && !rotationMode && target.parentNode._gsMaxScrollX && !_placeholderDiv.parentNode && !target.getBBox) {
        _placeholderDiv.style.width = target.parentNode.scrollWidth + "px";
        target.parentNode.appendChild(_placeholderDiv);
      }
      recordStartPositions();
      self.tween && self.tween.kill();
      self.isThrowing = false;
      gsap18.killTweensOf(scrollProxy || target, killProps, true);
      scrollProxy && gsap18.killTweensOf(target, {
        scrollTo: 1
      }, true);
      self.tween = self.lockedAxis = null;
      if (vars.zIndexBoost || !rotationMode && !scrollProxy && vars.zIndexBoost !== false) {
        target.style.zIndex = Draggable2.zIndex++;
      }
      self.isPressed = true;
      hasDragCallback = !!(vars.onDrag || self._listeners.drag);
      hasMoveCallback = !!(vars.onMove || self._listeners.move);
      if (vars.cursor !== false || vars.activeCursor) {
        i2 = triggers.length;
        while (--i2 > -1) {
          gsap18.set(triggers[i2], {
            cursor: vars.activeCursor || vars.cursor || (_defaultCursor === "grab" ? "grabbing" : _defaultCursor)
          });
        }
      }
      _dispatchEvent(self, "press", "onPress");
      InertiaPlugin && InertiaPlugin.track(scrollProxy || target, xyMode ? "x,y" : rotationMode ? "rotation" : "top,left");
    }, onMove = function onMove2(e) {
      var originalEvent = e, touches, pointerX, pointerY, i2, dx, dy;
      if (!enabled || _isMultiTouching || !self.isPressed || !e) {
        isPreventingDefault && e && enabled && _preventDefault(e);
        return;
      }
      self.pointerEvent = e;
      touches = e.changedTouches;
      if (touches) {
        e = touches[0];
        if (e !== touch && e.identifier !== touchID) {
          i2 = touches.length;
          while (--i2 > -1 && (e = touches[i2]).identifier !== touchID && e.target !== target) {
          }
          if (i2 < 0) {
            return;
          }
        }
      } else if (e.pointerId && touchID && e.pointerId !== touchID) {
        return;
      }
      if (touchEventTarget && allowNativeTouchScrolling && !touchDragAxis) {
        _point1.x = e.pageX - (isFixed ? _getDocScrollLeft3(ownerDoc) : 0);
        _point1.y = e.pageY - (isFixed ? _getDocScrollTop3(ownerDoc) : 0);
        matrix && matrix.apply(_point1, _point1);
        pointerX = _point1.x;
        pointerY = _point1.y;
        dx = Math.abs(pointerX - startPointerX);
        dy = Math.abs(pointerY - startPointerY);
        if (dx !== dy && (dx > minimumMovement || dy > minimumMovement) || _isAndroid && allowNativeTouchScrolling === touchDragAxis) {
          touchDragAxis = dx > dy && allowX ? "x" : "y";
          if (allowNativeTouchScrolling && touchDragAxis !== allowNativeTouchScrolling) {
            _addListener5(_win7, "touchforcechange", _preventDefault);
          }
          if (self.vars.lockAxisOnTouchScroll !== false && allowX && allowY) {
            self.lockedAxis = touchDragAxis === "x" ? "y" : "x";
            _isFunction9(self.vars.onLockAxis) && self.vars.onLockAxis.call(self, originalEvent);
          }
          if (_isAndroid && allowNativeTouchScrolling === touchDragAxis) {
            onRelease(originalEvent);
            return;
          }
        }
      }
      if (!self.allowEventDefault && (!allowNativeTouchScrolling || touchDragAxis && allowNativeTouchScrolling !== touchDragAxis) && originalEvent.cancelable !== false) {
        _preventDefault(originalEvent);
        isPreventingDefault = true;
      } else if (isPreventingDefault) {
        isPreventingDefault = false;
      }
      if (self.autoScroll) {
        checkAutoScrollBounds = true;
      }
      setPointerPosition(e.pageX, e.pageY, hasMoveCallback);
    }, setPointerPosition = function setPointerPosition2(pointerX, pointerY, invokeOnMove) {
      var dragTolerance = 1 - self.dragResistance, edgeTolerance = 1 - self.edgeResistance, prevPointerX = self.pointerX, prevPointerY = self.pointerY, prevStartElementY = startElementY, prevX = self.x, prevY = self.y, prevEndX = self.endX, prevEndY = self.endY, prevEndRotation = self.endRotation, prevDirty = dirty, xChange, yChange, x, y, dif, temp;
      self.pointerX = pointerX;
      self.pointerY = pointerY;
      if (isFixed) {
        pointerX -= _getDocScrollLeft3(ownerDoc);
        pointerY -= _getDocScrollTop3(ownerDoc);
      }
      if (rotationMode) {
        y = _round19(Math.atan2(rotationOrigin.y - pointerY, pointerX - rotationOrigin.x) * _RAD2DEG4);
        dif = self.y - y;
        if (dif > 180) {
          startElementY -= 360;
          self.y = y;
        } else if (dif < -180) {
          startElementY += 360;
          self.y = y;
        }
        if (matrix) {
          temp = pointerX * matrix.a + pointerY * matrix.c + matrix.e;
          pointerY = pointerX * matrix.b + pointerY * matrix.d + matrix.f;
          pointerX = temp;
        }
        if (self.x !== startElementX || Math.max(Math.abs(startPointerX - pointerX), Math.abs(startPointerY - pointerY)) > minimumMovement) {
          self.y = y;
          x = _round19(startElementX + (startElementY - y) * dragTolerance);
        } else {
          x = startElementX;
        }
      } else {
        if (matrix) {
          temp = pointerX * matrix.a + pointerY * matrix.c + matrix.e;
          pointerY = pointerX * matrix.b + pointerY * matrix.d + matrix.f;
          pointerX = temp;
        }
        yChange = pointerY - startPointerY;
        xChange = pointerX - startPointerX;
        if (yChange < minimumMovement && yChange > -minimumMovement) {
          yChange = 0;
        }
        if (xChange < minimumMovement && xChange > -minimumMovement) {
          xChange = 0;
        }
        if ((self.lockAxis || self.lockedAxis) && (xChange || yChange)) {
          temp = self.lockedAxis;
          if (!temp) {
            self.lockedAxis = temp = allowX && Math.abs(xChange) > Math.abs(yChange) ? "y" : allowY ? "x" : null;
            if (temp && _isFunction9(self.vars.onLockAxis)) {
              self.vars.onLockAxis.call(self, self.pointerEvent);
            }
          }
          if (temp === "y") {
            yChange = 0;
          } else if (temp === "x") {
            xChange = 0;
          }
        }
        x = _round19(startElementX + xChange * dragTolerance);
        y = _round19(startElementY + yChange * dragTolerance);
      }
      if ((snapX || snapY || snapXY) && (self.x !== x || self.y !== y && !rotationMode)) {
        if (snapXY) {
          _temp1.x = x;
          _temp1.y = y;
          temp = snapXY(_temp1);
          x = _round19(temp.x);
          y = _round19(temp.y);
        }
        if (snapX) {
          x = _round19(snapX(x));
        }
        if (snapY) {
          y = _round19(snapY(y));
        }
      }
      if (hasBounds) {
        if (x > maxX) {
          x = maxX + Math.round((x - maxX) * edgeTolerance);
        } else if (x < minX) {
          x = minX + Math.round((x - minX) * edgeTolerance);
        }
        if (!rotationMode) {
          if (y > maxY) {
            y = Math.round(maxY + (y - maxY) * edgeTolerance);
          } else if (y < minY) {
            y = Math.round(minY + (y - minY) * edgeTolerance);
          }
        }
      }
      if (self.x !== x || self.y !== y && !rotationMode) {
        if (rotationMode) {
          self.endRotation = self.x = self.endX = _round19(x);
          dirty = true;
        } else {
          if (allowY) {
            self.y = self.endY = y;
            dirty = true;
          }
          if (allowX) {
            self.x = self.endX = x;
            dirty = true;
          }
        }
        if (!invokeOnMove || _dispatchEvent(self, "move", "onMove") !== false) {
          if (!self.isDragging && self.isPressed) {
            self.isDragging = dragged = true;
            _dispatchEvent(self, "dragstart", "onDragStart");
          }
        } else {
          self.pointerX = prevPointerX;
          self.pointerY = prevPointerY;
          startElementY = prevStartElementY;
          self.x = prevX;
          self.y = prevY;
          self.endX = prevEndX;
          self.endY = prevEndY;
          self.endRotation = prevEndRotation;
          dirty = prevDirty;
        }
      }
    }, onRelease = function onRelease2(e, force) {
      if (!enabled || !self.isPressed || e && touchID != null && !force && (e.pointerId && e.pointerId !== touchID && e.target !== target || e.changedTouches && !_hasTouchID(e.changedTouches, touchID))) {
        isPreventingDefault && e && enabled && _preventDefault(e);
        return;
      }
      self.isPressed = false;
      var originalEvent = e, wasDragging = self.isDragging, isContextMenuRelease = self.vars.allowContextMenu && e && (e.ctrlKey || e.which > 2), placeholderDelayedCall = gsap18.delayedCall(1e-3, removePlaceholder), touches, i2, syntheticEvent, eventTarget, syntheticClick;
      if (touchEventTarget) {
        _removeListener5(touchEventTarget, "touchend", onRelease2);
        _removeListener5(touchEventTarget, "touchmove", onMove);
        _removeListener5(touchEventTarget, "touchcancel", onRelease2);
        _removeListener5(ownerDoc, "touchstart", _onMultiTouchDocument);
      } else {
        _removeListener5(ownerDoc, "mousemove", onMove);
      }
      _removeListener5(_win7, "touchforcechange", _preventDefault);
      if (!_supportsPointer || !touchEventTarget) {
        _removeListener5(ownerDoc, "mouseup", onRelease2);
        e && e.target && _removeListener5(e.target, "mouseup", onRelease2);
      }
      dirty = false;
      if (wasDragging) {
        dragEndTime = _lastDragTime = _getTime3();
        self.isDragging = false;
      }
      _removeFromRenderQueue(render14);
      if (isClicking && !isContextMenuRelease) {
        if (e) {
          _removeListener5(e.target, "change", onRelease2);
          self.pointerEvent = originalEvent;
        }
        _setSelectable(triggers, false);
        _dispatchEvent(self, "release", "onRelease");
        _dispatchEvent(self, "click", "onClick");
        isClicking = false;
        return;
      }
      i2 = triggers.length;
      while (--i2 > -1) {
        _setStyle(triggers[i2], "cursor", vars.cursor || (vars.cursor !== false ? _defaultCursor : null));
      }
      _dragCount2--;
      if (e) {
        touches = e.changedTouches;
        if (touches) {
          e = touches[0];
          if (e !== touch && e.identifier !== touchID) {
            i2 = touches.length;
            while (--i2 > -1 && (e = touches[i2]).identifier !== touchID && e.target !== target) {
            }
            if (i2 < 0 && !force) {
              return;
            }
          }
        }
        self.pointerEvent = originalEvent;
        self.pointerX = e.pageX;
        self.pointerY = e.pageY;
      }
      if (isContextMenuRelease && originalEvent) {
        _preventDefault(originalEvent);
        isPreventingDefault = true;
        _dispatchEvent(self, "release", "onRelease");
      } else if (originalEvent && !wasDragging) {
        isPreventingDefault = false;
        if (interrupted && (vars.snap || vars.bounds)) {
          animate(vars.inertia || vars.throwProps);
        }
        _dispatchEvent(self, "release", "onRelease");
        if ((!_isAndroid || originalEvent.type !== "touchmove") && originalEvent.type.indexOf("cancel") === -1) {
          _dispatchEvent(self, "click", "onClick");
          if (_getTime3() - clickTime < 300) {
            _dispatchEvent(self, "doubleclick", "onDoubleClick");
          }
          eventTarget = originalEvent.target || target;
          clickTime = _getTime3();
          syntheticClick = function syntheticClick2() {
            if (clickTime !== clickDispatch && self.enabled() && !self.isPressed && !originalEvent.defaultPrevented) {
              if (eventTarget.click) {
                eventTarget.click();
              } else if (ownerDoc.createEvent) {
                syntheticEvent = ownerDoc.createEvent("MouseEvents");
                syntheticEvent.initMouseEvent("click", true, true, _win7, 1, self.pointerEvent.screenX, self.pointerEvent.screenY, self.pointerX, self.pointerY, false, false, false, false, 0, null);
                eventTarget.dispatchEvent(syntheticEvent);
              }
            }
          };
          if (!_isAndroid && !originalEvent.defaultPrevented) {
            gsap18.delayedCall(0.05, syntheticClick);
          }
        }
      } else {
        animate(vars.inertia || vars.throwProps);
        if (!self.allowEventDefault && originalEvent && (vars.dragClickables !== false || !isClickable.call(self, originalEvent.target)) && wasDragging && (!allowNativeTouchScrolling || touchDragAxis && allowNativeTouchScrolling === touchDragAxis) && originalEvent.cancelable !== false) {
          isPreventingDefault = true;
          _preventDefault(originalEvent);
        } else {
          isPreventingDefault = false;
        }
        _dispatchEvent(self, "release", "onRelease");
      }
      isTweening2() && placeholderDelayedCall.duration(self.tween.duration());
      wasDragging && _dispatchEvent(self, "dragend", "onDragEnd");
      return true;
    }, updateScroll = function updateScroll2(e) {
      if (e && self.isDragging && !scrollProxy) {
        var parent = e.target || target.parentNode, deltaX = parent.scrollLeft - parent._gsScrollX, deltaY = parent.scrollTop - parent._gsScrollY;
        if (deltaX || deltaY) {
          if (matrix) {
            startPointerX -= deltaX * matrix.a + deltaY * matrix.c;
            startPointerY -= deltaY * matrix.d + deltaX * matrix.b;
          } else {
            startPointerX -= deltaX;
            startPointerY -= deltaY;
          }
          parent._gsScrollX += deltaX;
          parent._gsScrollY += deltaY;
          setPointerPosition(self.pointerX, self.pointerY);
        }
      }
    }, onClick = function onClick2(e) {
      var time = _getTime3(), recentlyClicked = time - clickTime < 100, recentlyDragged = time - dragEndTime < 50, alreadyDispatched = recentlyClicked && clickDispatch === clickTime, defaultPrevented = self.pointerEvent && self.pointerEvent.defaultPrevented, alreadyDispatchedTrusted = recentlyClicked && trustedClickDispatch === clickTime, trusted = e.isTrusted || e.isTrusted == null && recentlyClicked && alreadyDispatched;
      if ((alreadyDispatched || recentlyDragged && self.vars.suppressClickOnDrag !== false) && e.stopImmediatePropagation) {
        e.stopImmediatePropagation();
      }
      if (recentlyClicked && !(self.pointerEvent && self.pointerEvent.defaultPrevented) && (!alreadyDispatched || trusted && !alreadyDispatchedTrusted)) {
        if (trusted && alreadyDispatched) {
          trustedClickDispatch = clickTime;
        }
        clickDispatch = clickTime;
        return;
      }
      if (self.isPressed || recentlyDragged || recentlyClicked) {
        if (!trusted || !e.detail || !recentlyClicked || defaultPrevented) {
          _preventDefault(e);
        }
      }
      if (!recentlyClicked && !recentlyDragged && !dragged) {
        e && e.target && (self.pointerEvent = e);
        _dispatchEvent(self, "click", "onClick");
      }
    }, localizePoint = function localizePoint2(p2) {
      return matrix ? {
        x: p2.x * matrix.a + p2.y * matrix.c + matrix.e,
        y: p2.x * matrix.b + p2.y * matrix.d + matrix.f
      } : {
        x: p2.x,
        y: p2.y
      };
    };
    old = Draggable2.get(target);
    old && old.kill();
    _this2.startDrag = function(event, align) {
      var r1, r2, p1, p2;
      onPress(event || self.pointerEvent, true);
      if (align && !self.hitTest(event || self.pointerEvent)) {
        r1 = _parseRect(event || self.pointerEvent);
        r2 = _parseRect(target);
        p1 = localizePoint({
          x: r1.left + r1.width / 2,
          y: r1.top + r1.height / 2
        });
        p2 = localizePoint({
          x: r2.left + r2.width / 2,
          y: r2.top + r2.height / 2
        });
        startPointerX -= p1.x - p2.x;
        startPointerY -= p1.y - p2.y;
      }
      if (!self.isDragging) {
        self.isDragging = dragged = true;
        _dispatchEvent(self, "dragstart", "onDragStart");
      }
    };
    _this2.drag = onMove;
    _this2.endDrag = function(e) {
      return onRelease(e || self.pointerEvent, true);
    };
    _this2.timeSinceDrag = function() {
      return self.isDragging ? 0 : (_getTime3() - dragEndTime) / 1e3;
    };
    _this2.timeSinceClick = function() {
      return (_getTime3() - clickTime) / 1e3;
    };
    _this2.hitTest = function(target2, threshold) {
      return Draggable2.hitTest(self.target, target2, threshold);
    };
    _this2.getDirection = function(from, diagonalThreshold) {
      var mode = from === "velocity" && InertiaPlugin ? from : _isObject5(from) && !rotationMode ? "element" : "start", xChange, yChange, ratio, direction, r1, r2;
      if (mode === "element") {
        r1 = _parseRect(self.target);
        r2 = _parseRect(from);
      }
      xChange = mode === "start" ? self.x - startElementX : mode === "velocity" ? InertiaPlugin.getVelocity(target, xProp) : r1.left + r1.width / 2 - (r2.left + r2.width / 2);
      if (rotationMode) {
        return xChange < 0 ? "counter-clockwise" : "clockwise";
      } else {
        diagonalThreshold = diagonalThreshold || 2;
        yChange = mode === "start" ? self.y - startElementY : mode === "velocity" ? InertiaPlugin.getVelocity(target, yProp) : r1.top + r1.height / 2 - (r2.top + r2.height / 2);
        ratio = Math.abs(xChange / yChange);
        direction = ratio < 1 / diagonalThreshold ? "" : xChange < 0 ? "left" : "right";
        if (ratio < diagonalThreshold) {
          if (direction !== "") {
            direction += "-";
          }
          direction += yChange < 0 ? "up" : "down";
        }
      }
      return direction;
    };
    _this2.applyBounds = function(newBounds, sticky) {
      var x, y, forceZeroVelocity, e, parent, isRoot;
      if (newBounds && vars.bounds !== newBounds) {
        vars.bounds = newBounds;
        return self.update(true, sticky);
      }
      syncXY(true);
      calculateBounds();
      if (hasBounds && !isTweening2()) {
        x = self.x;
        y = self.y;
        if (x > maxX) {
          x = maxX;
        } else if (x < minX) {
          x = minX;
        }
        if (y > maxY) {
          y = maxY;
        } else if (y < minY) {
          y = minY;
        }
        if (self.x !== x || self.y !== y) {
          forceZeroVelocity = true;
          self.x = self.endX = x;
          if (rotationMode) {
            self.endRotation = x;
          } else {
            self.y = self.endY = y;
          }
          dirty = true;
          render14(true);
          if (self.autoScroll && !self.isDragging) {
            _recordMaxScrolls(target.parentNode);
            e = target;
            _windowProxy.scrollTop = _win7.pageYOffset != null ? _win7.pageYOffset : ownerDoc.documentElement.scrollTop != null ? ownerDoc.documentElement.scrollTop : ownerDoc.body.scrollTop;
            _windowProxy.scrollLeft = _win7.pageXOffset != null ? _win7.pageXOffset : ownerDoc.documentElement.scrollLeft != null ? ownerDoc.documentElement.scrollLeft : ownerDoc.body.scrollLeft;
            while (e && !isRoot) {
              isRoot = _isRoot(e.parentNode);
              parent = isRoot ? _windowProxy : e.parentNode;
              if (allowY && parent.scrollTop > parent._gsMaxScrollY) {
                parent.scrollTop = parent._gsMaxScrollY;
              }
              if (allowX && parent.scrollLeft > parent._gsMaxScrollX) {
                parent.scrollLeft = parent._gsMaxScrollX;
              }
              e = parent;
            }
          }
        }
        if (self.isThrowing && (forceZeroVelocity || self.endX > maxX || self.endX < minX || self.endY > maxY || self.endY < minY)) {
          animate(vars.inertia || vars.throwProps, forceZeroVelocity);
        }
      }
      return self;
    };
    _this2.update = function(applyBounds, sticky, ignoreExternalChanges) {
      if (sticky && self.isPressed) {
        if (rotationMode) {
          self.x = self.y = _round19(parseFloat(gsCache.rotation));
        } else {
          var m = getGlobalMatrix(target), p2 = innerMatrix.apply({
            x: self.x - startElementX,
            y: self.y - startElementY
          }), m2 = getGlobalMatrix(target.parentNode, true);
          m2.apply({
            x: m.e - p2.x,
            y: m.f - p2.y
          }, p2);
          self.x = _round19(self.x - (p2.x - m2.e));
          self.y = _round19(self.y - (p2.y - m2.f));
        }
        render14(true);
        recordStartPositions();
      }
      var x = self.x, y = self.y;
      updateMatrix(!sticky);
      if (applyBounds) {
        self.applyBounds();
      } else {
        dirty && ignoreExternalChanges && render14(true);
        syncXY(true);
      }
      if (sticky) {
        setPointerPosition(self.pointerX, self.pointerY);
        dirty && render14(true);
      }
      if (self.isPressed && !sticky && (allowX && Math.abs(x - self.x) > 0.01 || allowY && Math.abs(y - self.y) > 0.01 && !rotationMode)) {
        recordStartPositions();
      }
      if (self.autoScroll) {
        _recordMaxScrolls(target.parentNode, self.isDragging);
        checkAutoScrollBounds = self.isDragging;
        render14(true);
        _removeScrollListener(target, updateScroll);
        _addScrollListener(target, updateScroll);
      }
      return self;
    };
    _this2.enable = function(type2) {
      var setVars = {
        lazy: true
      }, id, i2, trigger;
      if (vars.cursor !== false) {
        setVars.cursor = vars.cursor || _defaultCursor;
      }
      if (gsap18.utils.checkPrefix("touchCallout")) {
        setVars.touchCallout = "none";
      }
      if (type2 !== "soft") {
        _setTouchActionForAllDescendants(triggers, allowX === allowY ? "none" : vars.allowNativeTouchScrolling && target.scrollHeight === target.clientHeight === (target.scrollWidth === target.clientHeight) || vars.allowEventDefault ? "manipulation" : allowX ? "pan-y" : "pan-x");
        i2 = triggers.length;
        while (--i2 > -1) {
          trigger = triggers[i2];
          _supportsPointer || _addListener5(trigger, "mousedown", onPress);
          _addListener5(trigger, "touchstart", onPress);
          _addListener5(trigger, "click", onClick, true);
          gsap18.set(trigger, setVars);
          if (trigger.getBBox && trigger.ownerSVGElement && allowX !== allowY) {
            gsap18.set(trigger.ownerSVGElement, {
              touchAction: vars.allowNativeTouchScrolling || vars.allowEventDefault ? "manipulation" : allowX ? "pan-y" : "pan-x"
            });
          }
          vars.allowContextMenu || _addListener5(trigger, "contextmenu", onContextMenu);
        }
        _setSelectable(triggers, false);
      }
      _addScrollListener(target, updateScroll);
      enabled = true;
      if (InertiaPlugin && type2 !== "soft") {
        InertiaPlugin.track(scrollProxy || target, xyMode ? "x,y" : rotationMode ? "rotation" : "top,left");
      }
      target._gsDragID = id = target._gsDragID || "d" + _lookupCount++;
      _lookup[id] = self;
      if (scrollProxy) {
        scrollProxy.enable();
        scrollProxy.element._gsDragID = id;
      }
      (vars.bounds || rotationMode) && recordStartPositions();
      vars.bounds && self.applyBounds();
      return self;
    };
    _this2.disable = function(type2) {
      var dragging = self.isDragging, i2 = triggers.length, trigger;
      while (--i2 > -1) {
        _setStyle(triggers[i2], "cursor", null);
      }
      if (type2 !== "soft") {
        _setTouchActionForAllDescendants(triggers, null);
        i2 = triggers.length;
        while (--i2 > -1) {
          trigger = triggers[i2];
          _setStyle(trigger, "touchCallout", null);
          _removeListener5(trigger, "mousedown", onPress);
          _removeListener5(trigger, "touchstart", onPress);
          _removeListener5(trigger, "click", onClick, true);
          _removeListener5(trigger, "contextmenu", onContextMenu);
        }
        _setSelectable(triggers, true);
        if (touchEventTarget) {
          _removeListener5(touchEventTarget, "touchcancel", onRelease);
          _removeListener5(touchEventTarget, "touchend", onRelease);
          _removeListener5(touchEventTarget, "touchmove", onMove);
        }
        _removeListener5(ownerDoc, "mouseup", onRelease);
        _removeListener5(ownerDoc, "mousemove", onMove);
      }
      _removeScrollListener(target, updateScroll);
      enabled = false;
      if (InertiaPlugin && type2 !== "soft") {
        InertiaPlugin.untrack(scrollProxy || target, xyMode ? "x,y" : rotationMode ? "rotation" : "top,left");
        self.tween && self.tween.kill();
      }
      scrollProxy && scrollProxy.disable();
      _removeFromRenderQueue(render14);
      self.isDragging = self.isPressed = isClicking = false;
      dragging && _dispatchEvent(self, "dragend", "onDragEnd");
      return self;
    };
    _this2.enabled = function(value, type2) {
      return arguments.length ? value ? self.enable(type2) : self.disable(type2) : enabled;
    };
    _this2.kill = function() {
      self.isThrowing = false;
      self.tween && self.tween.kill();
      self.disable();
      gsap18.set(triggers, {
        clearProps: "userSelect"
      });
      delete _lookup[target._gsDragID];
      return self;
    };
    _this2.revert = function() {
      this.kill();
      this.styles && this.styles.revert();
    };
    if (~type.indexOf("scroll")) {
      scrollProxy = _this2.scrollProxy = new ScrollProxy(target, _extend({
        onKill: function onKill() {
          self.isPressed && onRelease(null);
        }
      }, vars));
      target.style.overflowY = allowY && !_isTouchDevice ? "auto" : "hidden";
      target.style.overflowX = allowX && !_isTouchDevice ? "auto" : "hidden";
      target = scrollProxy.content;
    }
    if (rotationMode) {
      killProps.rotation = 1;
    } else {
      if (allowX) {
        killProps[xProp] = 1;
      }
      if (allowY) {
        killProps[yProp] = 1;
      }
    }
    gsCache.force3D = "force3D" in vars ? vars.force3D : true;
    _context5(_assertThisInitialized2(_this2));
    _this2.enable();
    return _this2;
  }
  Draggable2.register = function register8(core) {
    gsap18 = core;
    _initCore25();
  };
  Draggable2.create = function create(targets, vars) {
    _coreInitted15 || _initCore25(true);
    return _toArray8(targets).map(function(target) {
      return new Draggable2(target, vars);
    });
  };
  Draggable2.get = function get(target) {
    return _lookup[(_toArray8(target)[0] || {})._gsDragID];
  };
  Draggable2.timeSinceDrag = function timeSinceDrag() {
    return (_getTime3() - _lastDragTime) / 1e3;
  };
  Draggable2.hitTest = function hitTest(obj1, obj2, threshold) {
    if (obj1 === obj2) {
      return false;
    }
    var r1 = _parseRect(obj1), r2 = _parseRect(obj2), top = r1.top, left = r1.left, right = r1.right, bottom = r1.bottom, width = r1.width, height = r1.height, isOutside = r2.left > right || r2.right < left || r2.top > bottom || r2.bottom < top, overlap, area, isRatio;
    if (isOutside || !threshold) {
      return !isOutside;
    }
    isRatio = (threshold + "").indexOf("%") !== -1;
    threshold = parseFloat(threshold) || 0;
    overlap = {
      left: Math.max(left, r2.left),
      top: Math.max(top, r2.top)
    };
    overlap.width = Math.min(right, r2.right) - overlap.left;
    overlap.height = Math.min(bottom, r2.bottom) - overlap.top;
    if (overlap.width < 0 || overlap.height < 0) {
      return false;
    }
    if (isRatio) {
      threshold *= 0.01;
      area = overlap.width * overlap.height;
      return area >= width * height * threshold || area >= r2.width * r2.height * threshold;
    }
    return overlap.width > threshold && overlap.height > threshold;
  };
  return Draggable2;
})(EventDispatcher);
_setDefaults5(Draggable.prototype, {
  pointerX: 0,
  pointerY: 0,
  startX: 0,
  startY: 0,
  deltaX: 0,
  deltaY: 0,
  isDragging: false,
  isPressed: false
});
Draggable.zIndex = 1e3;
Draggable.version = "3.15.0";
_getGSAP29() && gsap18.registerPlugin(Draggable);

// examples/figma-launch/node_modules/gsap/utils/VelocityTracker.js
var gsap19;
var _coreInitted16;
var _toArray9;
var _getUnit4;
var _first;
var _ticker2;
var _time12;
var _time22;
var _getCache3;
var _getGSAP31 = function _getGSAP32() {
  return gsap19 || typeof window !== "undefined" && (gsap19 = window.gsap);
};
var _lookup2 = {};
var _round21 = function _round22(value) {
  return Math.round(value * 1e4) / 1e4;
};
var _getID3 = function _getID4(target) {
  return _getCache3(target).id;
};
var _getByTarget = function _getByTarget2(target) {
  return _lookup2[_getID3(typeof target === "string" ? _toArray9(target)[0] : target)];
};
var _onTick = function _onTick2(time) {
  var pt = _first, val;
  if (time - _time12 >= 0.05) {
    _time22 = _time12;
    _time12 = time;
    while (pt) {
      val = pt.g(pt.t, pt.p);
      if (val !== pt.v1 || time - pt.t1 > 0.2) {
        pt.v2 = pt.v1;
        pt.v1 = val;
        pt.t2 = pt.t1;
        pt.t1 = time;
      }
      pt = pt._next;
    }
  }
};
var _types2 = {
  deg: 360,
  rad: Math.PI * 2
};
var _initCore27 = function _initCore28() {
  gsap19 = _getGSAP31();
  if (gsap19) {
    _toArray9 = gsap19.utils.toArray;
    _getUnit4 = gsap19.utils.getUnit;
    _getCache3 = gsap19.core.getCache;
    _ticker2 = gsap19.ticker;
    _coreInitted16 = 1;
  }
};
var PropTracker = function PropTracker2(target, property, type, next) {
  this.t = target;
  this.p = property;
  this.g = target._gsap.get;
  this.rCap = _types2[type || _getUnit4(this.g(target, property))];
  this.v1 = this.v2 = this.g(target, property);
  this.t1 = this.t2 = _ticker2.time;
  if (next) {
    this._next = next;
    next._prev = this;
  }
};
var VelocityTracker = /* @__PURE__ */ (function() {
  function VelocityTracker2(target, property) {
    _coreInitted16 || _initCore27();
    this.target = _toArray9(target)[0];
    _lookup2[_getID3(this.target)] = this;
    this._props = {};
    property && this.add(property);
  }
  VelocityTracker2.register = function register8(core) {
    gsap19 = core;
    _initCore27();
  };
  var _proto = VelocityTracker2.prototype;
  _proto.get = function get(property, skipRecentTick) {
    var pt = this._props[property] || console.warn("Not tracking " + property + " velocity."), val, dif, rotationCap;
    val = parseFloat(skipRecentTick ? pt.v1 : pt.g(pt.t, pt.p));
    dif = val - parseFloat(pt.v2);
    rotationCap = pt.rCap;
    if (rotationCap) {
      dif = dif % rotationCap;
      if (dif !== dif % (rotationCap / 2)) {
        dif = dif < 0 ? dif + rotationCap : dif - rotationCap;
      }
    }
    return _round21(dif / ((skipRecentTick ? pt.t1 : _ticker2.time) - pt.t2));
  };
  _proto.getAll = function getAll() {
    var result = {}, props = this._props, p2;
    for (p2 in props) {
      result[p2] = this.get(p2);
    }
    return result;
  };
  _proto.isTracking = function isTracking(property) {
    return property in this._props;
  };
  _proto.add = function add(property, type) {
    var pt = this._props[property];
    if (pt) {
      pt.v1 = pt.v2 = pt.g(pt.t, pt.p);
      pt.t1 = pt.t2 = _ticker2.time;
    } else {
      if (!_first) {
        _ticker2.add(_onTick);
        _time12 = _time22 = _ticker2.time;
      }
      _first = this._props[property] = new PropTracker(this.target, property, type, _first);
    }
  };
  _proto.remove = function remove(property) {
    var pt = this._props[property], prev, next;
    if (pt) {
      prev = pt._prev;
      next = pt._next;
      if (prev) {
        prev._next = next;
      }
      if (next) {
        next._prev = prev;
      } else if (_first === pt) {
        _ticker2.remove(_onTick);
        _first = 0;
      }
      delete this._props[property];
    }
  };
  _proto.kill = function kill5(shallow) {
    for (var p2 in this._props) {
      this.remove(p2);
    }
    if (!shallow) {
      delete _lookup2[_getID3(this.target)];
    }
  };
  VelocityTracker2.track = function track(targets, properties, types) {
    _coreInitted16 || _initCore27();
    var result = [], targs = _toArray9(targets), a = properties.split(","), t = (types || "").split(","), i2 = targs.length, tracker, j;
    while (i2--) {
      tracker = _getByTarget(targs[i2]) || new VelocityTracker2(targs[i2]);
      j = a.length;
      while (j--) {
        tracker.add(a[j], t[j] || t[0]);
      }
      result.push(tracker);
    }
    return result;
  };
  VelocityTracker2.untrack = function untrack(targets, properties) {
    var props = properties && properties.split(",");
    _toArray9(targets).forEach(function(target) {
      var tracker = _getByTarget(target);
      if (tracker) {
        props ? props.forEach(function(p2) {
          return tracker.remove(p2);
        }) : tracker.kill(1);
      }
    });
  };
  VelocityTracker2.isTracking = function isTracking(target, property) {
    var tracker = _getByTarget(target);
    return tracker && tracker.isTracking(property);
  };
  VelocityTracker2.getVelocity = function getVelocity(target, property) {
    var tracker = _getByTarget(target);
    return !tracker || !tracker.isTracking(property) ? console.warn("Not tracking velocity of " + property) : tracker.get(property);
  };
  return VelocityTracker2;
})();
VelocityTracker.getByTarget = _getByTarget;
_getGSAP31() && gsap19.registerPlugin(VelocityTracker);

// examples/figma-launch/node_modules/gsap/InertiaPlugin.js
var gsap20;
var _coreInitted17;
var _parseEase5;
var _toArray10;
var _power3;
var _config3;
var _getUnit5;
var PropTween3;
var _getCache4;
var _checkPointRatio;
var _clamp5;
var _processingVars;
var _getStyleSaver9;
var _reverting8;
var _getTracker = VelocityTracker.getByTarget;
var _getGSAP33 = function _getGSAP34() {
  return gsap20 || typeof window !== "undefined" && (gsap20 = window.gsap) && gsap20.registerPlugin && gsap20;
};
var _isString9 = function _isString10(value) {
  return typeof value === "string";
};
var _isNumber7 = function _isNumber8(value) {
  return typeof value === "number";
};
var _isObject7 = function _isObject8(value) {
  return typeof value === "object";
};
var _isFunction11 = function _isFunction12(value) {
  return typeof value === "function";
};
var _bonusValidated6 = 1;
var _isArray3 = Array.isArray;
var _emptyFunc7 = function _emptyFunc8(p2) {
  return p2;
};
var _bigNum6 = 1e10;
var _tinyNum2 = 1 / _bigNum6;
var _checkPoint = 0.05;
var _round23 = function _round24(value) {
  return Math.round(value * 1e4) / 1e4;
};
var _extend3 = function _extend4(obj, defaults2, exclude) {
  for (var p2 in defaults2) {
    if (!(p2 in obj) && p2 !== exclude) {
      obj[p2] = defaults2[p2];
    }
  }
  return obj;
};
var _deepClone = function _deepClone2(obj) {
  var copy = {}, p2, v;
  for (p2 in obj) {
    copy[p2] = _isObject7(v = obj[p2]) && !_isArray3(v) ? _deepClone2(v) : v;
  }
  return copy;
};
var _getClosest = function _getClosest2(n, values, max, min, radius) {
  var i2 = values.length, closest = 0, absDif = _bigNum6, val, dif, p2, dist;
  if (_isObject7(n)) {
    while (i2--) {
      val = values[i2];
      dif = 0;
      for (p2 in n) {
        dist = val[p2] - n[p2];
        dif += dist * dist;
      }
      if (dif < absDif) {
        closest = i2;
        absDif = dif;
      }
    }
    if ((radius || _bigNum6) < _bigNum6 && radius < Math.sqrt(absDif)) {
      return n;
    }
  } else {
    while (i2--) {
      val = values[i2];
      dif = val - n;
      if (dif < 0) {
        dif = -dif;
      }
      if (dif < absDif && val >= min && val <= max) {
        closest = i2;
        absDif = dif;
      }
    }
  }
  return values[closest];
};
var _parseEnd = function _parseEnd2(curProp, end, max, min, name, radius, velocity) {
  if (curProp.end === "auto") {
    return curProp;
  }
  var endVar = curProp.end, adjustedEnd, p2;
  max = isNaN(max) ? _bigNum6 : max;
  min = isNaN(min) ? -_bigNum6 : min;
  if (_isObject7(end)) {
    adjustedEnd = end.calculated ? end : (_isFunction11(endVar) ? endVar(end, velocity) : _getClosest(end, endVar, max, min, radius)) || end;
    if (!end.calculated) {
      for (p2 in adjustedEnd) {
        end[p2] = adjustedEnd[p2];
      }
      end.calculated = true;
    }
    adjustedEnd = adjustedEnd[name];
  } else {
    adjustedEnd = _isFunction11(endVar) ? endVar(end, velocity) : _isArray3(endVar) ? _getClosest(end, endVar, max, min, radius) : parseFloat(endVar);
  }
  if (adjustedEnd > max) {
    adjustedEnd = max;
  } else if (adjustedEnd < min) {
    adjustedEnd = min;
  }
  return {
    max: adjustedEnd,
    min: adjustedEnd,
    unitFactor: curProp.unitFactor
  };
};
var _getNumOrDefault = function _getNumOrDefault2(vars, property, defaultValue) {
  return isNaN(vars[property]) ? defaultValue : +vars[property];
};
var _calculateChange = function _calculateChange2(velocity, duration) {
  return duration * _checkPoint * velocity / _checkPointRatio;
};
var _calculateDuration = function _calculateDuration2(start, end, velocity) {
  return Math.abs((end - start) * _checkPointRatio / velocity / _checkPoint);
};
var _reservedProps2 = {
  resistance: 1,
  checkpoint: 1,
  preventOvershoot: 1,
  linkedProps: 1,
  radius: 1,
  duration: 1
};
var _processLinkedProps = function _processLinkedProps2(target, vars, getVal, resistance) {
  if (vars.linkedProps) {
    var linkedPropNames = vars.linkedProps.split(","), linkedProps = {}, i2, p2, curProp, curVelocity, tracker, curDuration;
    for (i2 = 0; i2 < linkedPropNames.length; i2++) {
      p2 = linkedPropNames[i2];
      curProp = vars[p2];
      if (curProp) {
        if (_isNumber7(curProp.velocity)) {
          curVelocity = curProp.velocity;
        } else {
          tracker = tracker || _getTracker(target);
          curVelocity = tracker && tracker.isTracking(p2) ? tracker.get(p2) : 0;
        }
        curDuration = Math.abs(curVelocity / _getNumOrDefault(curProp, "resistance", resistance));
        linkedProps[p2] = parseFloat(getVal(target, p2)) + _calculateChange(curVelocity, curDuration);
      }
    }
    return linkedProps;
  }
};
var _calculateTweenDuration = function _calculateTweenDuration2(target, vars, maxDuration, minDuration, overshootTolerance, recordEnd) {
  if (maxDuration === void 0) {
    maxDuration = 10;
  }
  if (minDuration === void 0) {
    minDuration = 0.2;
  }
  if (overshootTolerance === void 0) {
    overshootTolerance = 1;
  }
  if (recordEnd === void 0) {
    recordEnd = 0;
  }
  _isString9(target) && (target = _toArray10(target)[0]);
  if (!target) {
    return 0;
  }
  var duration = 0, clippedDuration = _bigNum6, inertiaVars = vars.inertia || vars, getVal = _getCache4(target).get, resistance = _getNumOrDefault(inertiaVars, "resistance", _config3.resistance), p2, curProp, curDuration, curVelocity, curVal, end, curClippedDuration, tracker, unitFactor, linkedProps;
  linkedProps = _processLinkedProps(target, inertiaVars, getVal, resistance);
  for (p2 in inertiaVars) {
    if (!_reservedProps2[p2]) {
      curProp = inertiaVars[p2];
      if (!_isObject7(curProp)) {
        tracker = tracker || _getTracker(target);
        if (tracker && tracker.isTracking(p2)) {
          curProp = _isNumber7(curProp) ? {
            velocity: curProp
          } : {
            velocity: tracker.get(p2)
          };
        } else {
          curVelocity = +curProp || 0;
          curDuration = Math.abs(curVelocity / resistance);
        }
      }
      if (_isObject7(curProp)) {
        if (_isNumber7(curProp.velocity)) {
          curVelocity = curProp.velocity;
        } else {
          tracker = tracker || _getTracker(target);
          curVelocity = tracker && tracker.isTracking(p2) ? tracker.get(p2) : 0;
        }
        curDuration = _clamp5(minDuration, maxDuration, Math.abs(curVelocity / _getNumOrDefault(curProp, "resistance", resistance)));
        curVal = parseFloat(getVal(target, p2)) || 0;
        end = curVal + _calculateChange(curVelocity, curDuration);
        if ("end" in curProp) {
          curProp = _parseEnd(curProp, linkedProps && p2 in linkedProps ? linkedProps : end, curProp.max, curProp.min, p2, inertiaVars.radius, curVelocity);
          if (recordEnd) {
            _processingVars === vars && (_processingVars = inertiaVars = _deepClone(vars));
            inertiaVars[p2] = _extend3(curProp, inertiaVars[p2], "end");
          }
        }
        if ("max" in curProp && end > +curProp.max + _tinyNum2) {
          unitFactor = curProp.unitFactor || _config3.unitFactors[p2] || 1;
          curClippedDuration = curVal > curProp.max && curProp.min !== curProp.max || curVelocity * unitFactor > -15 && curVelocity * unitFactor < 45 ? minDuration + (maxDuration - minDuration) * 0.1 : _calculateDuration(curVal, curProp.max, curVelocity);
          if (curClippedDuration + overshootTolerance < clippedDuration) {
            clippedDuration = curClippedDuration + overshootTolerance;
          }
        } else if ("min" in curProp && end < +curProp.min - _tinyNum2) {
          unitFactor = curProp.unitFactor || _config3.unitFactors[p2] || 1;
          curClippedDuration = curVal < curProp.min && curProp.min !== curProp.max || curVelocity * unitFactor > -45 && curVelocity * unitFactor < 15 ? minDuration + (maxDuration - minDuration) * 0.1 : _calculateDuration(curVal, curProp.min, curVelocity);
          if (curClippedDuration + overshootTolerance < clippedDuration) {
            clippedDuration = curClippedDuration + overshootTolerance;
          }
        }
        curClippedDuration > duration && (duration = curClippedDuration);
      }
      curDuration > duration && (duration = curDuration);
    }
  }
  duration > clippedDuration && (duration = clippedDuration);
  return duration > maxDuration ? maxDuration : duration < minDuration ? minDuration : duration;
};
var _initCore29 = function _initCore30() {
  gsap20 = _getGSAP33();
  if (gsap20) {
    _parseEase5 = gsap20.parseEase;
    _toArray10 = gsap20.utils.toArray;
    _getUnit5 = gsap20.utils.getUnit;
    _getCache4 = gsap20.core.getCache;
    _clamp5 = gsap20.utils.clamp;
    _getStyleSaver9 = gsap20.core.getStyleSaver;
    _reverting8 = gsap20.core.reverting || function() {
    };
    _power3 = _parseEase5("power3");
    _checkPointRatio = _power3(0.05);
    PropTween3 = gsap20.core.PropTween;
    gsap20.config({
      resistance: 100,
      unitFactors: {
        time: 1e3,
        totalTime: 1e3,
        progress: 1e3,
        totalProgress: 1e3
      }
    });
    _config3 = gsap20.config();
    gsap20.registerPlugin(VelocityTracker);
    _coreInitted17 = 1;
  }
};
var InertiaPlugin2 = {
  version: "3.15.0",
  name: "inertia",
  register: function register6(core) {
    gsap20 = core;
    _initCore29();
  },
  init: function init12(target, vars, tween, index, targets) {
    _coreInitted17 || _initCore29();
    var tracker = _getTracker(target);
    if (vars === "auto") {
      if (!tracker) {
        console.warn("No inertia tracking on " + target + ". InertiaPlugin.track(target) first.");
        return;
      }
      vars = tracker.getAll();
    }
    this.styles = _getStyleSaver9 && typeof target.style === "object" && _getStyleSaver9(target);
    this.target = target;
    this.tween = tween;
    _processingVars = vars;
    var cache = target._gsap, getVal = cache.get, dur = vars.duration, durIsObj = _isObject7(dur), preventOvershoot = vars.preventOvershoot || durIsObj && dur.overshoot === 0, resistance = _getNumOrDefault(vars, "resistance", _config3.resistance), duration = _isNumber7(dur) ? dur : _calculateTweenDuration(target, vars, durIsObj && dur.max || 10, durIsObj && dur.min || 0.2, durIsObj && "overshoot" in dur ? +dur.overshoot : preventOvershoot ? 0 : 1, true), p2, curProp, curVal, unit, velocity, change1, end, change2, linkedProps;
    vars = _processingVars;
    _processingVars = 0;
    linkedProps = _processLinkedProps(target, vars, getVal, resistance);
    for (p2 in vars) {
      if (!_reservedProps2[p2]) {
        curProp = vars[p2];
        _isFunction11(curProp) && (curProp = curProp(index, target, targets));
        if (_isNumber7(curProp)) {
          velocity = curProp;
        } else if (_isObject7(curProp) && !isNaN(curProp.velocity)) {
          velocity = +curProp.velocity;
        } else {
          if (tracker && tracker.isTracking(p2)) {
            velocity = tracker.get(p2);
          } else {
            console.warn("ERROR: No velocity was defined for " + target + " property: " + p2);
          }
        }
        change1 = _calculateChange(velocity, duration);
        change2 = 0;
        curVal = getVal(target, p2);
        unit = _getUnit5(curVal);
        curVal = parseFloat(curVal);
        if (_isObject7(curProp)) {
          end = curVal + change1;
          if ("end" in curProp) {
            curProp = _parseEnd(curProp, linkedProps && p2 in linkedProps ? linkedProps : end, curProp.max, curProp.min, p2, vars.radius, velocity);
          }
          if ("max" in curProp && +curProp.max < end) {
            if (preventOvershoot || curProp.preventOvershoot) {
              change1 = curProp.max - curVal;
            } else {
              change2 = curProp.max - curVal - change1;
            }
          } else if ("min" in curProp && +curProp.min > end) {
            if (preventOvershoot || curProp.preventOvershoot) {
              change1 = curProp.min - curVal;
            } else {
              change2 = curProp.min - curVal - change1;
            }
          }
        }
        this._props.push(p2);
        this.styles && this.styles.save(p2);
        this._pt = new PropTween3(this._pt, target, p2, curVal, 0, _emptyFunc7, 0, cache.set(target, p2, this));
        this._pt.u = unit || 0;
        this._pt.c1 = change1;
        this._pt.c2 = change2;
      }
    }
    tween.duration(duration);
    return _bonusValidated6;
  },
  render: function render11(ratio, data) {
    var pt = data._pt;
    ratio = _power3(data.tween._time / data.tween._dur);
    if (ratio || !_reverting8()) {
      while (pt) {
        pt.set(pt.t, pt.p, _round23(pt.s + pt.c1 * ratio + pt.c2 * ratio * ratio) + pt.u, pt.d, ratio);
        pt = pt._next;
      }
    } else {
      data.styles.revert();
    }
  }
};
"track,untrack,isTracking,getVelocity,getByTarget".split(",").forEach(function(name) {
  return InertiaPlugin2[name] = VelocityTracker[name];
});
_getGSAP33() && gsap20.registerPlugin(InertiaPlugin2);

// examples/figma-launch/node_modules/gsap/ScrollSmoother.js
function _defineProperties2(target, props) {
  for (var i2 = 0; i2 < props.length; i2++) {
    var descriptor = props[i2];
    descriptor.enumerable = descriptor.enumerable || false;
    descriptor.configurable = true;
    if ("value" in descriptor) descriptor.writable = true;
    Object.defineProperty(target, descriptor.key, descriptor);
  }
}
function _createClass2(Constructor, protoProps, staticProps) {
  if (protoProps) _defineProperties2(Constructor.prototype, protoProps);
  if (staticProps) _defineProperties2(Constructor, staticProps);
  return Constructor;
}
var gsap21;
var _coreInitted18;
var _win8;
var _doc9;
var _docEl4;
var _body7;
var _root3;
var _toArray11;
var _clamp6;
var ScrollTrigger4;
var _mainInstance;
var _expo;
var _getVelocityProp3;
var _inputObserver3;
var _context6;
var _onResizeDelayedCall;
var _windowExists13 = function _windowExists14() {
  return typeof window !== "undefined";
};
var _getGSAP35 = function _getGSAP36() {
  return gsap21 || _windowExists13() && (gsap21 = window.gsap) && gsap21.registerPlugin && gsap21;
};
var _round25 = function _round26(value) {
  return Math.round(value * 1e5) / 1e5 || 0;
};
var _maxScroll3 = function _maxScroll4(scroller) {
  return ScrollTrigger4.maxScroll(scroller || _win8);
};
var _autoDistance = function _autoDistance2(el, progress) {
  var parent = el.parentNode || _docEl4, b1 = el.getBoundingClientRect(), b2 = parent.getBoundingClientRect(), gapTop = b2.top - b1.top, gapBottom = b2.bottom - b1.bottom, change = (Math.abs(gapTop) > Math.abs(gapBottom) ? gapTop : gapBottom) / (1 - progress), offset = -change * progress, ratio, extraChange;
  if (change > 0) {
    ratio = b2.height / (_win8.innerHeight + b2.height);
    extraChange = ratio === 0.5 ? b2.height * 2 : Math.min(b2.height, Math.abs(-change * ratio / (2 * ratio - 1))) * 2 * (progress || 1);
    offset += progress ? -extraChange * progress : -extraChange / 2;
    change += extraChange;
  }
  return {
    change,
    offset
  };
};
var _wrap = function _wrap2(el) {
  var wrapper = _doc9.querySelector(".ScrollSmoother-wrapper");
  if (!wrapper) {
    wrapper = _doc9.createElement("div");
    wrapper.classList.add("ScrollSmoother-wrapper");
    el.parentNode.insertBefore(wrapper, el);
    wrapper.appendChild(el);
  }
  return wrapper;
};
var ScrollSmoother = /* @__PURE__ */ (function() {
  function ScrollSmoother2(vars) {
    var _this = this;
    _coreInitted18 || ScrollSmoother2.register(gsap21) || console.warn("Please gsap.registerPlugin(ScrollSmoother)");
    vars = this.vars = vars || {};
    _mainInstance && _mainInstance.kill();
    _mainInstance = this;
    _context6(this);
    var _vars = vars, smoothTouch = _vars.smoothTouch, _onUpdate = _vars.onUpdate, onStop = _vars.onStop, smooth = _vars.smooth, onFocusIn = _vars.onFocusIn, normalizeScroll = _vars.normalizeScroll, wholePixels = _vars.wholePixels, content, wrapper, height, mainST, effects, sections, intervalID, wrapperCSS, contentCSS, paused, pausedNormalizer, recordedRefreshScroll, recordedRefreshScrub, allowUpdates, self = this, effectsPrefix = vars.effectsPrefix || "", scrollFunc = ScrollTrigger4.getScrollFunc(_win8), smoothDuration = ScrollTrigger4.isTouch === 1 ? smoothTouch === true ? 0.8 : parseFloat(smoothTouch) || 0 : smooth === 0 || smooth === false ? 0 : parseFloat(smooth) || 0.8, speed = smoothDuration && +vars.speed || 1, currentY = 0, delta = 0, startupPhase = 1, tracker = _getVelocityProp3(0), updateVelocity = function updateVelocity2() {
      return tracker.update(-currentY);
    }, scroll = {
      y: 0
    }, removeScroll = function removeScroll2() {
      return content.style.overflow = "visible";
    }, isProxyScrolling, killScrub = function killScrub2(trigger) {
      trigger.update();
      var scrub = trigger.getTween();
      if (scrub) {
        scrub.pause();
        scrub._time = scrub._dur;
        scrub._tTime = scrub._tDur;
      }
      isProxyScrolling = false;
      trigger.animation.progress(trigger.progress, true);
    }, render14 = function render15(y, force) {
      if (y !== currentY && !paused || force) {
        wholePixels && (y = Math.round(y));
        if (smoothDuration) {
          content.style.transform = "matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, " + y + ", 0, 1)";
          content._gsap.y = y + "px";
        }
        delta = y - currentY;
        currentY = y;
        ScrollTrigger4.isUpdating || ScrollSmoother2.isRefreshing || ScrollTrigger4.update();
      }
    }, scrollTop = function scrollTop2(value) {
      if (arguments.length) {
        value < 0 && (value = 0);
        scroll.y = -value;
        isProxyScrolling = true;
        paused ? currentY = -value : render14(-value);
        ScrollTrigger4.isRefreshing ? mainST.update() : scrollFunc(value / speed);
        return this;
      }
      return -currentY;
    }, resizeObserver = typeof ResizeObserver !== "undefined" && vars.autoResize !== false && new ResizeObserver(function() {
      if (!ScrollTrigger4.isRefreshing) {
        var max = _maxScroll3(wrapper) * speed;
        max < -currentY && scrollTop(max);
        _onResizeDelayedCall.restart(true);
      }
    }), lastFocusElement, _onFocusIn = function _onFocusIn2(e) {
      wrapper.scrollTop = 0;
      if (e.target.contains && e.target.contains(wrapper) || onFocusIn && onFocusIn(_this, e) === false) {
        return;
      }
      ScrollTrigger4.isInViewport(e.target) || e.target === lastFocusElement || _this.scrollTo(e.target, false, "center center");
      lastFocusElement = e.target;
    }, _transformPosition = function _transformPosition2(position, st) {
      if (position < st.start) {
        return position;
      }
      var ratio = isNaN(st.ratio) ? 1 : st.ratio, change = st.end - st.start, distance = position - st.start, offset = st.offset || 0, pins = st.pins || [], pinOffset = pins.offset || 0, progressOffset = st._startClamp && st.start <= 0 || st.pins && st.pins.offset ? 0 : st._endClamp && st.end === _maxScroll3() ? 1 : 0.5;
      pins.forEach(function(p2) {
        change -= p2.distance;
        if (p2.nativeStart <= position) {
          distance -= p2.distance;
        }
      });
      if (pinOffset) {
        distance *= (change - pinOffset / ratio) / change;
      }
      return position + (distance - offset * progressOffset) / ratio - distance;
    }, adjustEffectRelatedTriggers = function adjustEffectRelatedTriggers2(st, triggers, partial) {
      partial || (st.pins.length = st.pins.offset = 0);
      var pins = st.pins, markers = st.markers, dif, isClamped, start, end, nativeStart, nativeEnd, i2, trig;
      for (i2 = 0; i2 < triggers.length; i2++) {
        trig = triggers[i2];
        if (st.trigger && trig.trigger && st !== trig && (trig.trigger === st.trigger || trig.pinnedContainer === st.trigger || st.trigger.contains(trig.trigger))) {
          nativeStart = trig._startNative || trig._startClamp || trig.start;
          nativeEnd = trig._endNative || trig._endClamp || trig.end;
          start = _transformPosition(nativeStart, st);
          end = trig.pin && nativeEnd > 0 ? start + (nativeEnd - nativeStart) : _transformPosition(nativeEnd, st);
          trig.setPositions(start, end, true, (trig._startClamp ? Math.max(0, start) : start) - nativeStart);
          trig.markerStart && markers.push(gsap21.quickSetter([trig.markerStart, trig.markerEnd], "y", "px"));
          if (trig.pin && trig.end > 0 && !partial) {
            dif = trig.end - trig.start;
            isClamped = st._startClamp && trig.start < 0;
            if (isClamped) {
              if (st.start > 0) {
                st.setPositions(0, st.end + (st._startNative - st.start), true);
                adjustEffectRelatedTriggers2(st, triggers);
                return;
              }
              dif += trig.start;
              pins.offset = -trig.start;
            }
            pins.push({
              start: trig.start,
              nativeStart,
              end: trig.end,
              distance: dif,
              trig
            });
            st.setPositions(st.start, st.end + (isClamped ? -trig.start : dif), true);
          }
        }
      }
    }, adjustParallaxPosition = function adjustParallaxPosition2(triggers, createdAfterEffectWasApplied) {
      effects.forEach(function(st) {
        return adjustEffectRelatedTriggers(st, triggers, createdAfterEffectWasApplied);
      });
    }, onRefresh = function onRefresh2() {
      _docEl4 = _doc9.documentElement;
      _body7 = _doc9.body;
      removeScroll();
      requestAnimationFrame(removeScroll);
      if (effects) {
        ScrollTrigger4.getAll().forEach(function(st) {
          st._startNative = st.start;
          st._endNative = st.end;
        });
        effects.forEach(function(st) {
          var start = st._startClamp || st.start, end = st.autoSpeed ? Math.min(_maxScroll3(), st.end) : start + Math.abs((st.end - start) / st.ratio), offset = end - st.end;
          start -= offset / 2;
          end -= offset / 2;
          if (start > end) {
            var s = start;
            start = end;
            end = s;
          }
          if (st._startClamp && start < 0) {
            end = st.ratio < 0 ? _maxScroll3() : st.end / st.ratio;
            offset = end - st.end;
            start = 0;
          } else if (st.ratio < 0 || st._endClamp && end >= _maxScroll3()) {
            end = _maxScroll3();
            start = st.ratio < 0 ? 0 : st.ratio > 1 ? 0 : end - (end - st.start) / st.ratio;
            offset = (end - start) * st.ratio - (st.end - st.start);
          }
          st.offset = offset || 1e-4;
          st.pins.length = st.pins.offset = 0;
          st.setPositions(start, end, true);
        });
        adjustParallaxPosition(ScrollTrigger4.sort());
      }
      tracker.reset();
    }, addOnRefresh = function addOnRefresh2() {
      return ScrollTrigger4.addEventListener("refresh", onRefresh);
    }, restoreEffects = function restoreEffects2() {
      return effects && effects.forEach(function(st) {
        return st.vars.onRefresh(st);
      });
    }, revertEffects = function revertEffects2() {
      effects && effects.forEach(function(st) {
        return st.vars.onRefreshInit(st);
      });
      return restoreEffects;
    }, effectValueGetter = function effectValueGetter2(name, value, index, el) {
      return function() {
        var v = typeof value === "function" ? value(index, el) : value;
        v || v === 0 || (v = el.getAttribute("data-" + effectsPrefix + name) || (name === "speed" ? 1 : 0));
        el.setAttribute("data-" + effectsPrefix + name, v);
        var clamp3 = (v + "").substr(0, 6) === "clamp(";
        return {
          clamp: clamp3,
          value: clamp3 ? v.substr(6, v.length - 7) : v
        };
      };
    }, createEffect = function createEffect2(el, speed2, lag, index, effectsPadding) {
      effectsPadding = (typeof effectsPadding === "function" ? effectsPadding(index, el) : effectsPadding) || 0;
      var getSpeed = effectValueGetter("speed", speed2, index, el), getLag = effectValueGetter("lag", lag, index, el), startY = gsap21.getProperty(el, "y"), cache = el._gsap, ratio, st, autoSpeed, scrub, progressOffset, yOffset, pins = [], initDynamicValues = function initDynamicValues2() {
        speed2 = getSpeed();
        lag = parseFloat(getLag().value);
        ratio = parseFloat(speed2.value) || 1;
        autoSpeed = speed2.value === "auto";
        progressOffset = autoSpeed || st && st._startClamp && st.start <= 0 || pins.offset ? 0 : st && st._endClamp && st.end === _maxScroll3() ? 1 : 0.5;
        scrub && scrub.kill();
        scrub = lag && gsap21.to(el, {
          ease: _expo,
          overwrite: false,
          y: "+=0",
          duration: lag
        });
        if (st) {
          st.ratio = ratio;
          st.autoSpeed = autoSpeed;
        }
      }, revert = function revert2() {
        cache.y = startY + "px";
        cache.renderTransform(1);
        initDynamicValues();
      }, markers = [], change = 0, updateChange = function updateChange2(self2) {
        if (autoSpeed) {
          revert();
          var auto = _autoDistance(el, _clamp6(0, 1, -self2.start / (self2.end - self2.start)));
          change = auto.change;
          yOffset = auto.offset;
        } else {
          yOffset = pins.offset || 0;
          change = (self2.end - self2.start - yOffset) * (1 - ratio);
        }
        pins.forEach(function(p2) {
          return change -= p2.distance * (1 - ratio);
        });
        self2.offset = change || 1e-3;
        self2.vars.onUpdate(self2);
        scrub && scrub.progress(1);
      };
      initDynamicValues();
      if (ratio !== 1 || autoSpeed || scrub) {
        st = ScrollTrigger4.create({
          trigger: autoSpeed ? el.parentNode : el,
          start: function start() {
            return speed2.clamp ? "clamp(top bottom+=" + effectsPadding + ")" : "top bottom+=" + effectsPadding;
          },
          end: function end() {
            return speed2.value < 0 ? "max" : speed2.clamp ? "clamp(bottom top-=" + effectsPadding + ")" : "bottom top-=" + effectsPadding;
          },
          scroller: wrapper,
          scrub: true,
          refreshPriority: -999,
          // must update AFTER any other ScrollTrigger pins
          onRefreshInit: revert,
          onRefresh: updateChange,
          onKill: function onKill(self2) {
            var i2 = effects.indexOf(self2);
            i2 >= 0 && effects.splice(i2, 1);
            revert();
          },
          onUpdate: function onUpdate(self2) {
            var y = startY + change * (self2.progress - progressOffset), i2 = pins.length, extraY = 0, pin, scrollY, end;
            if (self2.offset) {
              if (i2) {
                scrollY = -currentY;
                end = self2.end;
                while (i2--) {
                  pin = pins[i2];
                  if (pin.trig.isActive || scrollY >= pin.start && scrollY <= pin.end) {
                    if (scrub) {
                      pin.trig.progress += pin.trig.direction < 0 ? 1e-3 : -1e-3;
                      pin.trig.update(0, 0, 1);
                      scrub.resetTo("y", parseFloat(cache.y), -delta, true);
                      startupPhase && scrub.progress(1);
                    }
                    return;
                  }
                  scrollY > pin.end && (extraY += pin.distance);
                  end -= pin.distance;
                }
                y = startY + extraY + change * ((gsap21.utils.clamp(self2.start, self2.end, scrollY) - self2.start - extraY) / (end - self2.start) - progressOffset);
              }
              markers.length && !autoSpeed && markers.forEach(function(setter) {
                return setter(y - extraY);
              });
              y = _round25(y + yOffset);
              if (scrub) {
                scrub.resetTo("y", y, -delta, true);
                startupPhase && scrub.progress(1);
              } else {
                cache.y = y + "px";
                cache.renderTransform(1);
              }
            }
          }
        });
        updateChange(st);
        gsap21.core.getCache(st.trigger).stRevert = revertEffects;
        st.startY = startY;
        st.pins = pins;
        st.markers = markers;
        st.ratio = ratio;
        st.autoSpeed = autoSpeed;
        el.style.willChange = "transform";
      }
      return st;
    };
    addOnRefresh();
    ScrollTrigger4.addEventListener("killAll", addOnRefresh);
    gsap21.delayedCall(0.5, function() {
      return startupPhase = 0;
    });
    this.scrollTop = scrollTop;
    this.scrollTo = function(target, smooth2, position) {
      var p2 = gsap21.utils.clamp(0, _maxScroll3(), isNaN(target) ? _this.offset(target, position, !!smooth2 && !paused) : +target);
      !smooth2 ? scrollTop(p2) : paused ? gsap21.to(_this, {
        duration: smoothDuration,
        scrollTop: p2,
        overwrite: "auto",
        ease: _expo
      }) : scrollFunc(p2);
    };
    this.offset = function(target, position, ignoreSpeed) {
      target = _toArray11(target)[0];
      var cssText = target.style.cssText, st = ScrollTrigger4.create({
        trigger: target,
        start: position || "top top"
      }), y;
      if (effects) {
        startupPhase ? ScrollTrigger4.refresh() : adjustParallaxPosition([st], true);
      }
      y = st.start / (ignoreSpeed ? speed : 1);
      st.kill(false);
      target.style.cssText = cssText;
      gsap21.core.getCache(target).uncache = 1;
      return y;
    };
    function refreshHeight() {
      height = content.clientHeight;
      content.style.overflow = "visible";
      _body7.style.height = _win8.innerHeight + (height - _win8.innerHeight) / speed + "px";
      return height - _win8.innerHeight;
    }
    this.content = function(element) {
      if (arguments.length) {
        var newContent = _toArray11(element || "#smooth-content")[0] || console.warn("ScrollSmoother needs a valid content element.") || _body7.children[0];
        if (newContent !== content) {
          content = newContent;
          contentCSS = content.getAttribute("style") || "";
          resizeObserver && resizeObserver.observe(content);
          gsap21.set(content, {
            overflow: "visible",
            width: "100%",
            boxSizing: "border-box",
            y: "+=0"
          });
          smoothDuration || gsap21.set(content, {
            clearProps: "transform"
          });
        }
        return this;
      }
      return content;
    };
    this.wrapper = function(element) {
      if (arguments.length) {
        wrapper = _toArray11(element || "#smooth-wrapper")[0] || _wrap(content);
        wrapperCSS = wrapper.getAttribute("style") || "";
        refreshHeight();
        gsap21.set(wrapper, smoothDuration ? {
          overflow: "hidden",
          position: "fixed",
          height: "100%",
          width: "100%",
          top: 0,
          left: 0,
          right: 0,
          bottom: 0
        } : {
          overflow: "visible",
          position: "relative",
          width: "100%",
          height: "auto",
          top: "auto",
          bottom: "auto",
          left: "auto",
          right: "auto"
        });
        return this;
      }
      return wrapper;
    };
    this.effects = function(targets, config3) {
      var _effects2;
      effects || (effects = []);
      if (!targets) {
        return effects.slice(0);
      }
      targets = _toArray11(targets);
      targets.forEach(function(target) {
        var i3 = effects.length;
        while (i3--) {
          effects[i3].trigger === target && effects[i3].kill();
        }
      });
      config3 = config3 || {};
      var _config4 = config3, speed2 = _config4.speed, lag = _config4.lag, effectsPadding = _config4.effectsPadding, effectsToAdd = [], i2, st;
      for (i2 = 0; i2 < targets.length; i2++) {
        st = createEffect(targets[i2], speed2, lag, i2, effectsPadding);
        st && effectsToAdd.push(st);
      }
      (_effects2 = effects).push.apply(_effects2, effectsToAdd);
      config3.refresh !== false && ScrollTrigger4.refresh();
      return effectsToAdd;
    };
    this.sections = function(targets, config3) {
      var _sections;
      sections || (sections = []);
      if (!targets) {
        return sections.slice(0);
      }
      var newSections = _toArray11(targets).map(function(el) {
        return ScrollTrigger4.create({
          trigger: el,
          start: "top 120%",
          end: "bottom -20%",
          onToggle: function onToggle(self2) {
            el.style.opacity = self2.isActive ? "1" : "0";
            el.style.pointerEvents = self2.isActive ? "all" : "none";
          }
        });
      });
      config3 && config3.add ? (_sections = sections).push.apply(_sections, newSections) : sections = newSections.slice(0);
      return newSections;
    };
    this.content(vars.content);
    this.wrapper(vars.wrapper);
    this.render = function(y) {
      return render14(y || y === 0 ? y : currentY);
    };
    this.getVelocity = function() {
      return tracker.getVelocity(-currentY);
    };
    ScrollTrigger4.scrollerProxy(wrapper, {
      scrollTop,
      scrollHeight: function scrollHeight() {
        return refreshHeight() && _body7.scrollHeight;
      },
      fixedMarkers: vars.fixedMarkers !== false && !!smoothDuration,
      content,
      getBoundingClientRect: function getBoundingClientRect() {
        return {
          top: 0,
          left: 0,
          width: _win8.innerWidth,
          height: _win8.innerHeight
        };
      }
    });
    ScrollTrigger4.defaults({
      scroller: wrapper
    });
    var existingScrollTriggers = ScrollTrigger4.getAll().filter(function(st) {
      return st.scroller === _win8 || st.scroller === wrapper;
    });
    existingScrollTriggers.forEach(function(st) {
      return st.revert(true, true);
    });
    mainST = ScrollTrigger4.create({
      animation: gsap21.fromTo(scroll, {
        y: function y() {
          allowUpdates = 0;
          return 0;
        }
      }, {
        y: function y() {
          allowUpdates = 1;
          return -refreshHeight();
        },
        immediateRender: false,
        ease: "none",
        data: "ScrollSmoother",
        duration: 100,
        // for added precision
        onUpdate: function onUpdate() {
          if (allowUpdates) {
            var force = isProxyScrolling;
            if (force) {
              killScrub(mainST);
              scroll.y = currentY;
            }
            render14(scroll.y, force);
            updateVelocity();
            _onUpdate && !paused && _onUpdate(self);
          }
        }
      }),
      onRefreshInit: function onRefreshInit(self2) {
        if (ScrollSmoother2.isRefreshing) {
          return;
        }
        ScrollSmoother2.isRefreshing = true;
        if (effects) {
          var _pins = ScrollTrigger4.getAll().filter(function(st) {
            return !!st.pin;
          });
          effects.forEach(function(st) {
            if (!st.vars.pinnedContainer) {
              _pins.forEach(function(pinST) {
                if (pinST.pin.contains(st.trigger)) {
                  var v = st.vars;
                  v.pinnedContainer = pinST.pin;
                  st.vars = null;
                  st.init(v, st.animation);
                }
              });
            }
          });
        }
        var scrub = self2.getTween();
        recordedRefreshScrub = scrub && scrub._end > scrub._dp._time;
        recordedRefreshScroll = currentY;
        scroll.y = 0;
        if (smoothDuration) {
          ScrollTrigger4.isTouch === 1 && (wrapper.style.position = "absolute");
          wrapper.scrollTop = 0;
          ScrollTrigger4.isTouch === 1 && (wrapper.style.position = "fixed");
        }
      },
      onRefresh: function onRefresh2(self2) {
        self2.animation.invalidate();
        scroll.y = 0;
        self2.setPositions(self2.start, refreshHeight() / speed);
        recordedRefreshScrub || killScrub(self2);
        scroll.y = -scrollFunc() * speed;
        render14(scroll.y);
        if (!startupPhase) {
          recordedRefreshScrub && (isProxyScrolling = false);
          self2.animation.progress(gsap21.utils.clamp(0, 1, recordedRefreshScroll / speed / -self2.end));
        }
        if (recordedRefreshScrub) {
          self2.progress -= 1e-3;
          self2.update();
        }
        ScrollSmoother2.isRefreshing = false;
      },
      id: "ScrollSmoother",
      scroller: _win8,
      invalidateOnRefresh: true,
      start: 0,
      refreshPriority: -9999,
      // because all other pins, etc. should be calculated first before this figures out the height of the body. BUT this should also update FIRST so that the scroll position on the proxy is up-to-date when all the ScrollTriggers calculate their progress! -9999 is a special number that ScrollTrigger looks for to handle in this way.
      end: function end() {
        return refreshHeight() / speed;
      },
      onScrubComplete: function onScrubComplete() {
        tracker.reset();
        onStop && onStop(_this);
      },
      scrub: smoothDuration || true
    });
    this.smooth = function(value) {
      if (arguments.length) {
        smoothDuration = value || 0;
        speed = smoothDuration && +vars.speed || 1;
        mainST.scrubDuration(value);
      }
      return mainST.getTween() ? mainST.getTween().duration() : 0;
    };
    mainST.getTween() && (mainST.getTween().vars.ease = vars.ease || _expo);
    this.scrollTrigger = mainST;
    vars.effects && this.effects(vars.effects === true ? "[data-" + effectsPrefix + "speed], [data-" + effectsPrefix + "lag]" : vars.effects, {
      effectsPadding: vars.effectsPadding,
      refresh: false
    });
    vars.sections && this.sections(vars.sections === true ? "[data-section]" : vars.sections);
    existingScrollTriggers.forEach(function(st) {
      st.vars.scroller = wrapper;
      st.revert(false, true);
      st.init(st.vars, st.animation);
    });
    this.paused = function(value, allowNestedScroll) {
      if (arguments.length) {
        if (!!paused !== value) {
          if (value) {
            mainST.getTween() && mainST.getTween().pause();
            scrollFunc(-currentY / speed);
            tracker.reset();
            pausedNormalizer = ScrollTrigger4.normalizeScroll();
            pausedNormalizer && pausedNormalizer.disable();
            paused = ScrollTrigger4.observe({
              preventDefault: true,
              type: "wheel,touch,scroll",
              debounce: false,
              allowClicks: true,
              onChangeY: function onChangeY() {
                return scrollTop(-currentY);
              }
              // refuse to scroll
            });
            paused.nested = _inputObserver3(_docEl4, "wheel,touch,scroll", true, allowNestedScroll !== false);
          } else {
            paused.nested.kill();
            paused.kill();
            paused = 0;
            pausedNormalizer && pausedNormalizer.enable();
            mainST.progress = (-currentY / speed - mainST.start) / (mainST.end - mainST.start);
            killScrub(mainST);
          }
        }
        return this;
      }
      return !!paused;
    };
    this.kill = this.revert = function() {
      _this.paused(false);
      killScrub(mainST);
      mainST.kill();
      var triggers = (effects || []).concat(sections || []), i2 = triggers.length;
      while (i2--) {
        triggers[i2].kill();
      }
      ScrollTrigger4.scrollerProxy(wrapper);
      ScrollTrigger4.removeEventListener("killAll", addOnRefresh);
      ScrollTrigger4.removeEventListener("refresh", onRefresh);
      wrapper.style.cssText = wrapperCSS;
      content.style.cssText = contentCSS;
      var defaults2 = ScrollTrigger4.defaults({});
      defaults2 && defaults2.scroller === wrapper && ScrollTrigger4.defaults({
        scroller: _win8
      });
      _this.normalizer && ScrollTrigger4.normalizeScroll(false);
      clearInterval(intervalID);
      _mainInstance = null;
      resizeObserver && resizeObserver.disconnect();
      _body7.style.removeProperty("height");
      _win8.removeEventListener("focusin", _onFocusIn);
    };
    this.refresh = function(soft, force) {
      return mainST.refresh(soft, force);
    };
    if (normalizeScroll) {
      this.normalizer = ScrollTrigger4.normalizeScroll(normalizeScroll === true ? {
        debounce: true,
        content: !smoothDuration && content
      } : normalizeScroll);
    }
    ScrollTrigger4.config(vars);
    "scrollBehavior" in _win8.getComputedStyle(_body7) && gsap21.set([_body7, _docEl4], {
      scrollBehavior: "auto"
    });
    _win8.addEventListener("focusin", _onFocusIn);
    intervalID = setInterval(updateVelocity, 250);
    _doc9.readyState === "loading" || requestAnimationFrame(function() {
      return ScrollTrigger4.refresh();
    });
  }
  ScrollSmoother2.register = function register8(core) {
    if (!_coreInitted18) {
      gsap21 = core || _getGSAP35();
      if (_windowExists13() && window.document) {
        _win8 = window;
        _doc9 = document;
        _docEl4 = _doc9.documentElement;
        _body7 = _doc9.body;
      }
      if (gsap21) {
        _toArray11 = gsap21.utils.toArray;
        _clamp6 = gsap21.utils.clamp;
        _expo = gsap21.parseEase("expo");
        _context6 = gsap21.core.context || function() {
        };
        ScrollTrigger4 = gsap21.core.globals().ScrollTrigger;
        gsap21.core.globals("ScrollSmoother", ScrollSmoother2);
        if (_body7 && ScrollTrigger4) {
          _onResizeDelayedCall = gsap21.delayedCall(0.2, function() {
            return ScrollTrigger4.isRefreshing || _mainInstance && _mainInstance.refresh();
          }).pause();
          _root3 = [_win8, _doc9, _docEl4, _body7];
          _getVelocityProp3 = ScrollTrigger4.core._getVelocityProp;
          _inputObserver3 = ScrollTrigger4.core._inputObserver;
          ScrollSmoother2.refresh = ScrollTrigger4.refresh;
          _coreInitted18 = 1;
        }
      }
    }
    return _coreInitted18;
  };
  _createClass2(ScrollSmoother2, [{
    key: "progress",
    get: function get() {
      return this.scrollTrigger ? this.scrollTrigger.animation._time / 100 : 0;
    }
  }]);
  return ScrollSmoother2;
})();
ScrollSmoother.version = "3.15.0";
ScrollSmoother.create = function(vars) {
  return _mainInstance && vars && _mainInstance.content() === _toArray11(vars.content)[0] ? _mainInstance : new ScrollSmoother(vars);
};
ScrollSmoother.get = function() {
  return _mainInstance;
};
_getGSAP35() && gsap21.registerPlugin(ScrollSmoother);

// examples/figma-launch/node_modules/gsap/PixiPlugin.js
var gsap22;
var _splitColor;
var _coreInitted19;
var _PIXI;
var PropTween4;
var _getSetter3;
var _isV4;
var _isV8Plus;
var _windowExists15 = function _windowExists16() {
  return typeof window !== "undefined";
};
var _getGSAP37 = function _getGSAP38() {
  return gsap22 || _windowExists15() && (gsap22 = window.gsap) && gsap22.registerPlugin && gsap22;
};
var _isFunction13 = function _isFunction14(value) {
  return typeof value === "function";
};
var _warn5 = function _warn6(message) {
  return console.warn(message);
};
var _idMatrix = [1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0];
var _lumR = 0.212671;
var _lumG = 0.71516;
var _lumB = 0.072169;
var _filterClass = function _filterClass2(name) {
  return _isFunction13(_PIXI[name]) ? _PIXI[name] : _PIXI.filters[name];
};
var _applyMatrix = function _applyMatrix2(m, m2) {
  var temp = [], i2 = 0, z = 0, y, x;
  for (y = 0; y < 4; y++) {
    for (x = 0; x < 5; x++) {
      z = x === 4 ? m[i2 + 4] : 0;
      temp[i2 + x] = m[i2] * m2[x] + m[i2 + 1] * m2[x + 5] + m[i2 + 2] * m2[x + 10] + m[i2 + 3] * m2[x + 15] + z;
    }
    i2 += 5;
  }
  return temp;
};
var _setSaturation = function _setSaturation2(m, n) {
  var inv = 1 - n, r = inv * _lumR, g = inv * _lumG, b = inv * _lumB;
  return _applyMatrix([r + n, g, b, 0, 0, r, g + n, b, 0, 0, r, g, b + n, 0, 0, 0, 0, 0, 1, 0], m);
};
var _colorize = function _colorize2(m, color, amount) {
  var c = _splitColor(color), r = c[0] / 255, g = c[1] / 255, b = c[2] / 255, inv = 1 - amount;
  return _applyMatrix([inv + amount * r * _lumR, amount * r * _lumG, amount * r * _lumB, 0, 0, amount * g * _lumR, inv + amount * g * _lumG, amount * g * _lumB, 0, 0, amount * b * _lumR, amount * b * _lumG, inv + amount * b * _lumB, 0, 0, 0, 0, 0, 1, 0], m);
};
var _setHue = function _setHue2(m, n) {
  n *= Math.PI / 180;
  var c = Math.cos(n), s = Math.sin(n);
  return _applyMatrix([_lumR + c * (1 - _lumR) + s * -_lumR, _lumG + c * -_lumG + s * -_lumG, _lumB + c * -_lumB + s * (1 - _lumB), 0, 0, _lumR + c * -_lumR + s * 0.143, _lumG + c * (1 - _lumG) + s * 0.14, _lumB + c * -_lumB + s * -0.283, 0, 0, _lumR + c * -_lumR + s * -(1 - _lumR), _lumG + c * -_lumG + s * _lumG, _lumB + c * (1 - _lumB) + s * _lumB, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1], m);
};
var _setContrast = function _setContrast2(m, n) {
  return _applyMatrix([n, 0, 0, 0, 0.5 * (1 - n), 0, n, 0, 0, 0.5 * (1 - n), 0, 0, n, 0, 0.5 * (1 - n), 0, 0, 0, 1, 0], m);
};
var _getFilter = function _getFilter2(target, type) {
  var filterClass = _filterClass(type), filters = target.filters || [], i2 = filters.length, filter;
  filterClass || _warn5(type + " not found. PixiPlugin.registerPIXI(PIXI)");
  while (--i2 > -1) {
    if (filters[i2] instanceof filterClass) {
      return filters[i2];
    }
  }
  filter = new filterClass();
  if (type === "BlurFilter") {
    if (_isV8Plus) {
      filter.strength = 0;
    } else {
      filter.blur = 0;
    }
  }
  target.filters = [].concat(filters, [filter]);
  return filter;
};
var _addColorMatrixFilterCacheTween = function _addColorMatrixFilterCacheTween2(p2, plugin, cache, vars) {
  plugin.add(cache, p2, cache[p2], vars[p2]);
  plugin._props.push(p2);
};
var _applyBrightnessToMatrix = function _applyBrightnessToMatrix2(brightness, matrix) {
  var filterClass = _filterClass("ColorMatrixFilter"), temp = new filterClass();
  temp.matrix = matrix;
  temp.brightness(brightness, true);
  return temp.matrix;
};
var _copy5 = function _copy6(obj) {
  var copy = {}, p2;
  for (p2 in obj) {
    copy[p2] = obj[p2];
  }
  return copy;
};
var _CMFdefaults = {
  contrast: 1,
  saturation: 1,
  colorizeAmount: 0,
  colorize: "rgb(255,255,255)",
  hue: 0,
  brightness: 1
};
var _parseColorMatrixFilter = function _parseColorMatrixFilter2(target, v, pg) {
  var filter = _getFilter(target, "ColorMatrixFilter"), cache = target._gsColorMatrixFilter = target._gsColorMatrixFilter || _copy5(_CMFdefaults), combine = v.combineCMF && !("colorMatrixFilter" in v && !v.colorMatrixFilter), i2, matrix, startMatrix;
  startMatrix = filter.matrix;
  if (v.resolution) {
    filter.resolution = v.resolution;
  }
  if (v.matrix && v.matrix.length === startMatrix.length) {
    matrix = v.matrix;
    if (cache.contrast !== 1) {
      _addColorMatrixFilterCacheTween("contrast", pg, cache, _CMFdefaults);
    }
    if (cache.hue) {
      _addColorMatrixFilterCacheTween("hue", pg, cache, _CMFdefaults);
    }
    if (cache.brightness !== 1) {
      _addColorMatrixFilterCacheTween("brightness", pg, cache, _CMFdefaults);
    }
    if (cache.colorizeAmount) {
      _addColorMatrixFilterCacheTween("colorize", pg, cache, _CMFdefaults);
      _addColorMatrixFilterCacheTween("colorizeAmount", pg, cache, _CMFdefaults);
    }
    if (cache.saturation !== 1) {
      _addColorMatrixFilterCacheTween("saturation", pg, cache, _CMFdefaults);
    }
  } else {
    matrix = _idMatrix.slice();
    if (v.contrast != null) {
      matrix = _setContrast(matrix, +v.contrast);
      _addColorMatrixFilterCacheTween("contrast", pg, cache, v);
    } else if (cache.contrast !== 1) {
      if (combine) {
        matrix = _setContrast(matrix, cache.contrast);
      } else {
        _addColorMatrixFilterCacheTween("contrast", pg, cache, _CMFdefaults);
      }
    }
    if (v.hue != null) {
      matrix = _setHue(matrix, +v.hue);
      _addColorMatrixFilterCacheTween("hue", pg, cache, v);
    } else if (cache.hue) {
      if (combine) {
        matrix = _setHue(matrix, cache.hue);
      } else {
        _addColorMatrixFilterCacheTween("hue", pg, cache, _CMFdefaults);
      }
    }
    if (v.brightness != null) {
      matrix = _applyBrightnessToMatrix(+v.brightness, matrix);
      _addColorMatrixFilterCacheTween("brightness", pg, cache, v);
    } else if (cache.brightness !== 1) {
      if (combine) {
        matrix = _applyBrightnessToMatrix(cache.brightness, matrix);
      } else {
        _addColorMatrixFilterCacheTween("brightness", pg, cache, _CMFdefaults);
      }
    }
    if (v.colorize != null) {
      v.colorizeAmount = "colorizeAmount" in v ? +v.colorizeAmount : 1;
      matrix = _colorize(matrix, v.colorize, v.colorizeAmount);
      _addColorMatrixFilterCacheTween("colorize", pg, cache, v);
      _addColorMatrixFilterCacheTween("colorizeAmount", pg, cache, v);
    } else if (cache.colorizeAmount) {
      if (combine) {
        matrix = _colorize(matrix, cache.colorize, cache.colorizeAmount);
      } else {
        _addColorMatrixFilterCacheTween("colorize", pg, cache, _CMFdefaults);
        _addColorMatrixFilterCacheTween("colorizeAmount", pg, cache, _CMFdefaults);
      }
    }
    if (v.saturation != null) {
      matrix = _setSaturation(matrix, +v.saturation);
      _addColorMatrixFilterCacheTween("saturation", pg, cache, v);
    } else if (cache.saturation !== 1) {
      if (combine) {
        matrix = _setSaturation(matrix, cache.saturation);
      } else {
        _addColorMatrixFilterCacheTween("saturation", pg, cache, _CMFdefaults);
      }
    }
  }
  i2 = matrix.length;
  while (--i2 > -1) {
    if (matrix[i2] !== startMatrix[i2]) {
      pg.add(startMatrix, i2, startMatrix[i2], matrix[i2], "colorMatrixFilter");
    }
  }
  pg._props.push("colorMatrixFilter");
};
var _renderColor = function _renderColor2(ratio, _ref) {
  var t = _ref.t, p2 = _ref.p, color = _ref.color, set = _ref.set;
  set(t, p2, color[0] << 16 | color[1] << 8 | color[2]);
};
var _renderDirtyCache = function _renderDirtyCache2(ratio, _ref2) {
  var g = _ref2.g;
  if (_isV8Plus) {
    g.fill();
    g.stroke();
  } else if (g) {
    g.dirty++;
    g.clearDirty++;
  }
};
var _renderAutoAlpha = function _renderAutoAlpha2(ratio, data) {
  data.t.visible = !!data.t.alpha;
};
var _addColorTween = function _addColorTween2(target, p2, value, plugin) {
  var currentValue = target[p2], startColor = _splitColor(_isFunction13(currentValue) ? target[p2.indexOf("set") || !_isFunction13(target["get" + p2.substr(3)]) ? p2 : "get" + p2.substr(3)]() : currentValue), endColor = _splitColor(value);
  plugin._pt = new PropTween4(plugin._pt, target, p2, 0, 0, _renderColor, {
    t: target,
    p: p2,
    color: startColor,
    set: _getSetter3(target, p2)
  });
  plugin.add(startColor, 0, startColor[0], endColor[0]);
  plugin.add(startColor, 1, startColor[1], endColor[1]);
  plugin.add(startColor, 2, startColor[2], endColor[2]);
};
var _colorProps = {
  tint: 1,
  lineColor: 1,
  fillColor: 1,
  strokeColor: 1
};
var _xyContexts = "position,scale,skew,pivot,anchor,tilePosition,tileScale".split(",");
var _contexts = {
  x: "position",
  y: "position",
  tileX: "tilePosition",
  tileY: "tilePosition"
};
var _colorMatrixFilterProps = {
  colorMatrixFilter: 1,
  saturation: 1,
  contrast: 1,
  hue: 1,
  colorize: 1,
  colorizeAmount: 1,
  brightness: 1,
  combineCMF: 1
};
var _DEG2RAD6 = Math.PI / 180;
var _isString11 = function _isString12(value) {
  return typeof value === "string";
};
var _degreesToRadians = function _degreesToRadians2(value) {
  return _isString11(value) && value.charAt(1) === "=" ? value.substr(0, 2) + parseFloat(value.substr(2)) * _DEG2RAD6 : value * _DEG2RAD6;
};
var _renderPropWithEnd3 = function _renderPropWithEnd4(ratio, data) {
  return data.set(data.t, data.p, ratio === 1 ? data.e : Math.round((data.s + data.c * ratio) * 1e5) / 1e5, data);
};
var _addRotationalPropTween3 = function _addRotationalPropTween4(plugin, target, property, startNum, endValue, radians) {
  var cap = 360 * (radians ? _DEG2RAD6 : 1), isString = _isString11(endValue), relative = isString && endValue.charAt(1) === "=" ? +(endValue.charAt(0) + "1") : 0, endNum = parseFloat(relative ? endValue.substr(2) : endValue) * (radians ? _DEG2RAD6 : 1), change = relative ? endNum * relative : endNum - startNum, finalValue = startNum + change, direction, pt;
  if (isString) {
    direction = endValue.split("_")[1];
    if (direction === "short") {
      change %= cap;
      if (change !== change % (cap / 2)) {
        change += change < 0 ? cap : -cap;
      }
    }
    if (direction === "cw" && change < 0) {
      change = (change + cap * 1e10) % cap - ~~(change / cap) * cap;
    } else if (direction === "ccw" && change > 0) {
      change = (change - cap * 1e10) % cap - ~~(change / cap) * cap;
    }
  }
  plugin._pt = pt = new PropTween4(plugin._pt, target, property, startNum, change, _renderPropWithEnd3);
  pt.e = finalValue;
  return pt;
};
var _initCore31 = function _initCore32() {
  if (!_coreInitted19) {
    gsap22 = _getGSAP37();
    _PIXI = _coreInitted19 = _PIXI || _windowExists15() && window.PIXI;
    var version = _PIXI && _PIXI.VERSION && parseFloat(_PIXI.VERSION.split(".")[0]) || 0;
    _isV4 = version === 4;
    _isV8Plus = version >= 8;
    _splitColor = function _splitColor2(color) {
      return gsap22.utils.splitColor((color + "").substr(0, 2) === "0x" ? "#" + color.substr(2) : color);
    };
  }
};
var i;
var p;
for (i = 0; i < _xyContexts.length; i++) {
  p = _xyContexts[i];
  _contexts[p + "X"] = p;
  _contexts[p + "Y"] = p;
}
var PixiPlugin = {
  version: "3.15.0",
  name: "pixi",
  register: function register7(core, Plugin, propTween) {
    gsap22 = core;
    PropTween4 = propTween;
    _getSetter3 = Plugin.getSetter;
    _initCore31();
  },
  headless: true,
  // doesn't need window
  registerPIXI: function registerPIXI(pixi) {
    _PIXI = pixi;
  },
  init: function init13(target, values, tween, index, targets) {
    _PIXI || _initCore31();
    if (!_PIXI) {
      _warn5("PIXI was not found. PixiPlugin.registerPIXI(PIXI);");
      return false;
    }
    var context3, axis, value, colorMatrix, filter, p2, padding, i2, data, subProp;
    for (p2 in values) {
      context3 = _contexts[p2];
      value = values[p2];
      if (context3) {
        axis = ~p2.charAt(p2.length - 1).toLowerCase().indexOf("x") ? "x" : "y";
        this.add(target[context3], axis, target[context3][axis], context3 === "skew" ? _degreesToRadians(value) : value, 0, 0, 0, 0, 0, 1);
      } else if (p2 === "scale" || p2 === "anchor" || p2 === "pivot" || p2 === "tileScale") {
        this.add(target[p2], "x", target[p2].x, value);
        this.add(target[p2], "y", target[p2].y, value);
      } else if (p2 === "rotation" || p2 === "angle") {
        _addRotationalPropTween3(this, target, p2, target[p2], value, p2 === "rotation");
      } else if (_colorMatrixFilterProps[p2]) {
        if (!colorMatrix) {
          _parseColorMatrixFilter(target, values.colorMatrixFilter || values, this);
          colorMatrix = true;
        }
      } else if (p2 === "blur" || p2 === "blurX" || p2 === "blurY" || p2 === "blurPadding") {
        filter = _getFilter(target, "BlurFilter");
        this.add(filter, p2, filter[p2], value);
        if (values.blurPadding !== 0) {
          padding = values.blurPadding || Math.max(filter[p2], value) * 2;
          i2 = target.filters.length;
          while (--i2 > -1) {
            target.filters[i2].padding = Math.max(target.filters[i2].padding, padding);
          }
        }
      } else if (_colorProps[p2]) {
        if ((p2 === "lineColor" || p2 === "fillColor" || p2 === "strokeColor") && target instanceof _PIXI.Graphics) {
          data = "fillStyle" in target ? [target] : (target.geometry || target).graphicsData;
          subProp = p2.substr(0, p2.length - 5);
          _isV8Plus && subProp === "line" && (subProp = "stroke");
          this._pt = new PropTween4(this._pt, target, p2, 0, 0, _renderDirtyCache, {
            g: target.geometry || target
          });
          i2 = data.length;
          while (--i2 > -1) {
            _addColorTween(_isV4 ? data[i2] : data[i2][subProp + "Style"], _isV4 ? p2 : "color", value, this);
          }
        } else {
          _addColorTween(target, p2, value, this);
        }
      } else if (p2 === "autoAlpha") {
        this._pt = new PropTween4(this._pt, target, "visible", 0, 0, _renderAutoAlpha);
        this.add(target, "alpha", target.alpha, value);
        this._props.push("alpha", "visible");
      } else if (p2 !== "resolution") {
        this.add(target, p2, "get", value);
      }
      this._props.push(p2);
    }
  }
};
_getGSAP37() && gsap22.registerPlugin(PixiPlugin);

// examples/figma-launch/node_modules/gsap/EaselPlugin.js
var gsap23;
var _coreInitted20;
var _win9;
var _createJS;
var _ColorFilter;
var _ColorMatrixFilter;
var _colorProps2 = "redMultiplier,greenMultiplier,blueMultiplier,alphaMultiplier,redOffset,greenOffset,blueOffset,alphaOffset".split(",");
var _windowExists17 = function _windowExists18() {
  return typeof window !== "undefined";
};
var _getGSAP39 = function _getGSAP40() {
  return gsap23 || _windowExists17() && (gsap23 = window.gsap) && gsap23.registerPlugin && gsap23;
};
var _getCreateJS = function _getCreateJS2() {
  return _createJS || _win9 && _win9.createjs || _win9 || {};
};
var _warn7 = function _warn8(message) {
  return console.warn(message);
};
var _cache = function _cache2(target) {
  var b = target.getBounds && target.getBounds();
  if (!b) {
    b = target.nominalBounds || {
      x: 0,
      y: 0,
      width: 100,
      height: 100
    };
    target.setBounds && target.setBounds(b.x, b.y, b.width, b.height);
  }
  target.cache && target.cache(b.x, b.y, b.width, b.height);
  _warn7("EaselPlugin: for filters to display in EaselJS, you must call the object's cache() method first. GSAP attempted to use the target's getBounds() for the cache but that may not be completely accurate. " + target);
};
var _parseColorFilter = function _parseColorFilter2(target, v, plugin) {
  if (!_ColorFilter) {
    _ColorFilter = _getCreateJS().ColorFilter;
    if (!_ColorFilter) {
      _warn7("EaselPlugin error: The EaselJS ColorFilter JavaScript file wasn't loaded.");
    }
  }
  var filters = target.filters || [], i2 = filters.length, c, s, e, a, p2, pt;
  while (i2--) {
    if (filters[i2] instanceof _ColorFilter) {
      s = filters[i2];
      break;
    }
  }
  if (!s) {
    s = new _ColorFilter();
    filters.push(s);
    target.filters = filters;
  }
  e = s.clone();
  if (v.tint != null) {
    c = gsap23.utils.splitColor(v.tint);
    a = v.tintAmount != null ? +v.tintAmount : 1;
    e.redOffset = +c[0] * a;
    e.greenOffset = +c[1] * a;
    e.blueOffset = +c[2] * a;
    e.redMultiplier = e.greenMultiplier = e.blueMultiplier = 1 - a;
  } else {
    for (p2 in v) {
      if (p2 !== "exposure") {
        if (p2 !== "brightness") {
          e[p2] = +v[p2];
        }
      }
    }
  }
  if (v.exposure != null) {
    e.redOffset = e.greenOffset = e.blueOffset = 255 * (+v.exposure - 1);
    e.redMultiplier = e.greenMultiplier = e.blueMultiplier = 1;
  } else if (v.brightness != null) {
    a = +v.brightness - 1;
    e.redOffset = e.greenOffset = e.blueOffset = a > 0 ? a * 255 : 0;
    e.redMultiplier = e.greenMultiplier = e.blueMultiplier = 1 - Math.abs(a);
  }
  i2 = 8;
  while (i2--) {
    p2 = _colorProps2[i2];
    if (s[p2] !== e[p2]) {
      pt = plugin.add(s, p2, s[p2], e[p2], 0, 0, 0, 0, 0, 1);
      if (pt) {
        pt.op = "easel_colorFilter";
      }
    }
  }
  plugin._props.push("easel_colorFilter");
  if (!target.cacheID) {
    _cache(target);
  }
};
var _idMatrix2 = [1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0];
var _lumR2 = 0.212671;
var _lumG2 = 0.71516;
var _lumB2 = 0.072169;
var _applyMatrix3 = function _applyMatrix4(m, m2) {
  if (!(m instanceof Array) || !(m2 instanceof Array)) {
    return m2;
  }
  var temp = [], i2 = 0, z = 0, y, x;
  for (y = 0; y < 4; y++) {
    for (x = 0; x < 5; x++) {
      z = x === 4 ? m[i2 + 4] : 0;
      temp[i2 + x] = m[i2] * m2[x] + m[i2 + 1] * m2[x + 5] + m[i2 + 2] * m2[x + 10] + m[i2 + 3] * m2[x + 15] + z;
    }
    i2 += 5;
  }
  return temp;
};
var _setSaturation3 = function _setSaturation4(m, n) {
  if (isNaN(n)) {
    return m;
  }
  var inv = 1 - n, r = inv * _lumR2, g = inv * _lumG2, b = inv * _lumB2;
  return _applyMatrix3([r + n, g, b, 0, 0, r, g + n, b, 0, 0, r, g, b + n, 0, 0, 0, 0, 0, 1, 0], m);
};
var _colorize3 = function _colorize4(m, color, amount) {
  if (isNaN(amount)) {
    amount = 1;
  }
  var c = gsap23.utils.splitColor(color), r = c[0] / 255, g = c[1] / 255, b = c[2] / 255, inv = 1 - amount;
  return _applyMatrix3([inv + amount * r * _lumR2, amount * r * _lumG2, amount * r * _lumB2, 0, 0, amount * g * _lumR2, inv + amount * g * _lumG2, amount * g * _lumB2, 0, 0, amount * b * _lumR2, amount * b * _lumG2, inv + amount * b * _lumB2, 0, 0, 0, 0, 0, 1, 0], m);
};
var _setHue3 = function _setHue4(m, n) {
  if (isNaN(n)) {
    return m;
  }
  n *= Math.PI / 180;
  var c = Math.cos(n), s = Math.sin(n);
  return _applyMatrix3([_lumR2 + c * (1 - _lumR2) + s * -_lumR2, _lumG2 + c * -_lumG2 + s * -_lumG2, _lumB2 + c * -_lumB2 + s * (1 - _lumB2), 0, 0, _lumR2 + c * -_lumR2 + s * 0.143, _lumG2 + c * (1 - _lumG2) + s * 0.14, _lumB2 + c * -_lumB2 + s * -0.283, 0, 0, _lumR2 + c * -_lumR2 + s * -(1 - _lumR2), _lumG2 + c * -_lumG2 + s * _lumG2, _lumB2 + c * (1 - _lumB2) + s * _lumB2, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1], m);
};
var _setContrast3 = function _setContrast4(m, n) {
  if (isNaN(n)) {
    return m;
  }
  n += 0.01;
  return _applyMatrix3([n, 0, 0, 0, 128 * (1 - n), 0, n, 0, 0, 128 * (1 - n), 0, 0, n, 0, 128 * (1 - n), 0, 0, 0, 1, 0], m);
};
var _parseColorMatrixFilter3 = function _parseColorMatrixFilter4(target, v, plugin) {
  if (!_ColorMatrixFilter) {
    _ColorMatrixFilter = _getCreateJS().ColorMatrixFilter;
    if (!_ColorMatrixFilter) {
      _warn7("EaselPlugin: The EaselJS ColorMatrixFilter JavaScript file wasn't loaded.");
    }
  }
  var filters = target.filters || [], i2 = filters.length, matrix, startMatrix, s, pg;
  while (--i2 > -1) {
    if (filters[i2] instanceof _ColorMatrixFilter) {
      s = filters[i2];
      break;
    }
  }
  if (!s) {
    s = new _ColorMatrixFilter(_idMatrix2.slice());
    filters.push(s);
    target.filters = filters;
  }
  startMatrix = s.matrix;
  matrix = _idMatrix2.slice();
  if (v.colorize != null) {
    matrix = _colorize3(matrix, v.colorize, Number(v.colorizeAmount));
  }
  if (v.contrast != null) {
    matrix = _setContrast3(matrix, Number(v.contrast));
  }
  if (v.hue != null) {
    matrix = _setHue3(matrix, Number(v.hue));
  }
  if (v.saturation != null) {
    matrix = _setSaturation3(matrix, Number(v.saturation));
  }
  i2 = matrix.length;
  while (--i2 > -1) {
    if (matrix[i2] !== startMatrix[i2]) {
      pg = plugin.add(startMatrix, i2, startMatrix[i2], matrix[i2], 0, 0, 0, 0, 0, 1);
      if (pg) {
        pg.op = "easel_colorMatrixFilter";
      }
    }
  }
  plugin._props.push("easel_colorMatrixFilter");
  if (!target.cacheID) {
    _cache();
  }
  plugin._matrix = startMatrix;
};
var _initCore33 = function _initCore34(core) {
  gsap23 = core || _getGSAP39();
  if (_windowExists17()) {
    _win9 = window;
  }
  if (gsap23) {
    _coreInitted20 = 1;
  }
};
var EaselPlugin = {
  version: "3.15.0",
  name: "easel",
  init: function init14(target, value, tween, index, targets) {
    if (!_coreInitted20) {
      _initCore33();
      if (!gsap23) {
        _warn7("Please gsap.registerPlugin(EaselPlugin)");
      }
    }
    this.target = target;
    var p2, pt, tint, colorMatrix, end, labels, i2;
    for (p2 in value) {
      end = value[p2];
      if (p2 === "colorFilter" || p2 === "tint" || p2 === "tintAmount" || p2 === "exposure" || p2 === "brightness") {
        if (!tint) {
          _parseColorFilter(target, value.colorFilter || value, this);
          tint = true;
        }
      } else if (p2 === "saturation" || p2 === "contrast" || p2 === "hue" || p2 === "colorize" || p2 === "colorizeAmount") {
        if (!colorMatrix) {
          _parseColorMatrixFilter3(target, value.colorMatrixFilter || value, this);
          colorMatrix = true;
        }
      } else if (p2 === "frame") {
        if (typeof end === "string" && end.charAt(1) !== "=" && (labels = target.labels)) {
          for (i2 = 0; i2 < labels.length; i2++) {
            if (labels[i2].label === end) {
              end = labels[i2].position;
            }
          }
        }
        pt = this.add(target, "gotoAndStop", target.currentFrame, end, index, targets, Math.round, 0, 0, 1);
        if (pt) {
          pt.op = p2;
        }
      } else if (target[p2] != null) {
        this.add(target, p2, "get", end);
      }
    }
  },
  render: function render12(ratio, data) {
    var pt = data._pt;
    while (pt) {
      pt.r(ratio, pt.d);
      pt = pt._next;
    }
    if (data.target.cacheID) {
      data.target.updateCache();
    }
  },
  register: _initCore33
};
EaselPlugin.registerCreateJS = function(createjs) {
  _createJS = createjs;
};
_getGSAP39() && gsap23.registerPlugin(EaselPlugin);

// examples/figma-launch/node_modules/gsap/CSSRulePlugin.js
var gsap24;
var _coreInitted21;
var _win10;
var _doc10;
var CSSPlugin2;
var _windowExists19 = function _windowExists20() {
  return typeof window !== "undefined";
};
var _getGSAP41 = function _getGSAP42() {
  return gsap24 || _windowExists19() && (gsap24 = window.gsap) && gsap24.registerPlugin && gsap24;
};
var _checkRegister = function _checkRegister2() {
  if (!_coreInitted21) {
    _initCore35();
    if (!CSSPlugin2) {
      console.warn("Please gsap.registerPlugin(CSSPlugin, CSSRulePlugin)");
    }
  }
  return _coreInitted21;
};
var _initCore35 = function _initCore36(core) {
  gsap24 = core || _getGSAP41();
  if (_windowExists19()) {
    _win10 = window;
    _doc10 = document;
  }
  if (gsap24) {
    CSSPlugin2 = gsap24.plugins.css;
    if (CSSPlugin2) {
      _coreInitted21 = 1;
    }
  }
};
var CSSRulePlugin = {
  version: "3.15.0",
  name: "cssRule",
  init: function init15(target, value, tween, index, targets) {
    if (!_checkRegister() || typeof target.cssText === "undefined") {
      return false;
    }
    var div = target._gsProxy = target._gsProxy || _doc10.createElement("div");
    this.ss = target;
    this.style = div.style;
    div.style.cssText = target.cssText;
    CSSPlugin2.prototype.init.call(this, div, value, tween, index, targets);
  },
  render: function render13(ratio, data) {
    var pt = data._pt, style = data.style, ss2 = data.ss, i2;
    while (pt) {
      pt.r(ratio, pt.d);
      pt = pt._next;
    }
    i2 = style.length;
    while (--i2 > -1) {
      ss2[style[i2]] = style[style[i2]];
    }
  },
  getRule: function getRule(selector3) {
    _checkRegister();
    var ruleProp = _doc10.all ? "rules" : "cssRules", styleSheets = _doc10.styleSheets, i2 = styleSheets.length, pseudo = selector3.charAt(0) === ":", j, curSS, cs, a;
    selector3 = (pseudo ? "" : ",") + selector3.split("::").join(":").toLowerCase() + ",";
    if (pseudo) {
      a = [];
    }
    while (i2--) {
      try {
        curSS = styleSheets[i2][ruleProp];
        if (!curSS) {
          continue;
        }
        j = curSS.length;
      } catch (e) {
        console.warn(e);
        continue;
      }
      while (--j > -1) {
        cs = curSS[j];
        if (cs.selectorText && ("," + cs.selectorText.split("::").join(":").toLowerCase() + ",").indexOf(selector3) !== -1) {
          if (pseudo) {
            a.push(cs.style);
          } else {
            return cs.style;
          }
        }
      }
    }
    return a;
  },
  register: _initCore35
};
_getGSAP41() && gsap24.registerPlugin(CSSRulePlugin);

// examples/figma-launch/node_modules/gsap/utils/PathEditor.js
var _numbersExp2 = /(?:(-)?\d*\.?\d*(?:e[\-+]?\d+)?)[0-9]/ig;
var _doc11;
var _supportsPointer2;
var _win11;
var _body8;
var gsap25;
var _context7;
var _selectionColor = "#4e7fff";
var _minimumMovement = 1;
var _DEG2RAD7 = Math.PI / 180;
var _getTime4 = Date.now || function() {
  return (/* @__PURE__ */ new Date()).getTime();
};
var _lastInteraction = 0;
var _isPressed = 0;
var _emptyFunc9 = function _emptyFunc10() {
  return false;
};
var _interacted = function _interacted2() {
  return _lastInteraction = _getTime4();
};
var _CTRL;
var _ALT;
var _SHIFT;
var _CMD;
var _recentlyAddedAnchor;
var _editingAxis = {};
var _history = [];
var _point = {};
var _temp3 = [];
var _comma = ",";
var _selectedPaths = [];
var _preventDefault3 = function _preventDefault4(event) {
  if (event.preventDefault) {
    event.preventDefault();
    if (event.preventManipulation) {
      event.preventManipulation();
    }
  }
};
var _createElement5 = function _createElement6(type) {
  return _doc11.createElementNS ? _doc11.createElementNS("http://www.w3.org/1999/xhtml", type) : _doc11.createElement(type);
};
var _createSVG = function _createSVG2(type, container, attributes) {
  var element = _doc11.createElementNS("http://www.w3.org/2000/svg", type), reg = /([a-z])([A-Z])/g, p2;
  attributes = attributes || {};
  attributes["class"] = attributes["class"] || "path-editor";
  for (p2 in attributes) {
    if (element.style[p2] !== void 0) {
      element.style[p2] = attributes[p2];
    } else {
      element.setAttributeNS(null, p2.replace(reg, "$1-$2").toLowerCase(), attributes[p2]);
    }
  }
  container.appendChild(element);
  return element;
};
var _identityMatrixObject = {
  matrix: new Matrix2D()
};
var _getConsolidatedMatrix = function _getConsolidatedMatrix2(target) {
  return (target.transform && target.transform.baseVal.consolidate() || _identityMatrixObject).matrix;
};
var _getConcatenatedTransforms = function _getConcatenatedTransforms2(target) {
  var m = _getConsolidatedMatrix(target), owner = target.ownerSVGElement;
  while ((target = target.parentNode) && target.ownerSVGElement === owner) {
    m.multiply(_getConsolidatedMatrix(target));
  }
  return "matrix(" + m.a + "," + m.b + "," + m.c + "," + m.d + "," + m.e + "," + m.f + ")";
};
var _addHistory = function _addHistory2(pathEditor) {
  var selectedIndexes = [], a = pathEditor._selectedAnchors, i2;
  for (i2 = 0; i2 < a.length; i2++) {
    selectedIndexes[i2] = a[i2].i;
  }
  _history.unshift({
    path: pathEditor,
    d: pathEditor.path.getAttribute("d"),
    transform: pathEditor.path.getAttribute("transform") || "",
    selectedIndexes
  });
  if (_history.length > 30) {
    _history.length = 30;
  }
};
var _round27 = function _round28(value) {
  return ~~(value * 1e3 + (value < 0 ? -0.5 : 0.5)) / 1e3;
};
var _getSquarePathData = function _getSquarePathData2(size) {
  size = _round27(size);
  return ["M-" + size, -size, size, -size, size, size, -size, size + "z"].join(_comma);
};
var _getCirclePathData = function _getCirclePathData2(size) {
  var circ = 0.552284749831, rcirc = _round27(size * circ);
  size = _round27(size);
  return "M" + size + ",0C" + [size, rcirc, rcirc, size, 0, size, -rcirc, size, -size, rcirc, -size, 0, -size, -rcirc, -rcirc, -size, 0, -size, rcirc, -size, size, -rcirc, size, 0].join(_comma) + "z";
};
var _checkDeselect = function _checkDeselect2(e) {
  if (!e.target._gsSelection && !_isPressed && _getTime4() - _lastInteraction > 100) {
    var i2 = _selectedPaths.length;
    while (--i2 > -1) {
      _selectedPaths[i2].deselect();
    }
    _selectedPaths.length = 0;
  }
};
var _tempDiv4;
var _touchEventLookup2;
var _isMultiTouching2 = 0;
var _addListener7 = function _addListener8(element, type, func, capture) {
  if (element.addEventListener) {
    var touchType = _touchEventLookup2[type];
    capture = capture || {
      passive: false
    };
    element.addEventListener(touchType || type, func, capture);
    if (touchType && type !== touchType && touchType.substr(0, 7) !== "pointer") {
      element.addEventListener(type, func, capture);
    }
  } else if (element.attachEvent) {
    element.attachEvent("on" + type, func);
  }
};
var _removeListener7 = function _removeListener8(element, type, func) {
  if (element.removeEventListener) {
    var touchType = _touchEventLookup2[type];
    element.removeEventListener(touchType || type, func);
    if (touchType && type !== touchType && touchType.substr(0, 7) !== "pointer") {
      element.removeEventListener(type, func);
    }
  } else if (element.detachEvent) {
    element.detachEvent("on" + type, func);
  }
};
var _hasTouchID3 = function _hasTouchID4(list, ID) {
  var i2 = list.length;
  while (--i2 > -1) {
    if (list[i2].identifier === ID) {
      return true;
    }
  }
  return false;
};
var _onMultiTouchDocumentEnd3 = function _onMultiTouchDocumentEnd4(e) {
  _isMultiTouching2 = e.touches && _dragCount < e.touches.length;
  _removeListener7(e.target, "touchend", _onMultiTouchDocumentEnd4);
};
var _onMultiTouchDocument3 = function _onMultiTouchDocument4(e) {
  _isMultiTouching2 = e.touches && _dragCount < e.touches.length;
  _addListener7(e.target, "touchend", _onMultiTouchDocumentEnd3);
};
var _bind = function _bind2(func, scope) {
  return function(e) {
    return func.call(scope, e);
  };
};
var _callback5 = function _callback6(type, self, param) {
  var callback = self.vars[type];
  if (callback) {
    callback.call(self.vars.callbackScope || self, param || self);
  }
  return self;
};
var _copyElement;
var _resetSelection = function _resetSelection2() {
  _copyElement.style.display = "block";
  _copyElement.select();
  _copyElement.style.display = "none";
};
var _coreInitted22;
var _initCore37 = function _initCore38(core) {
  _doc11 = document;
  _win11 = window;
  _body8 = _doc11.body;
  gsap25 = gsap25 || core || _win11.gsap || console.warn("Please gsap.registerPlugin(PathEditor)");
  _context7 = gsap25 && gsap25.core.context || function() {
  };
  _tempDiv4 = _createElement5("div");
  _copyElement = _createElement5("textarea");
  _copyElement.style.display = "none";
  _body8 && _body8.appendChild(_copyElement);
  _touchEventLookup2 = (function(types) {
    var standard = types.split(","), converted = (_tempDiv4.onpointerdown !== void 0 ? "pointerdown,pointermove,pointerup,pointercancel" : _tempDiv4.onmspointerdown !== void 0 ? "MSPointerDown,MSPointerMove,MSPointerUp,MSPointerCancel" : types).split(","), obj = {}, i2 = 4;
    while (--i2 > -1) {
      obj[standard[i2]] = converted[i2];
      obj[converted[i2]] = standard[i2];
    }
    return obj;
  })("touchstart,touchmove,touchend,touchcancel");
  SVGElement.prototype.getTransformToElement = SVGElement.prototype.getTransformToElement || function(e) {
    return e.getScreenCTM().inverse().multiply(this.getScreenCTM());
  };
  _doc11.addEventListener("keydown", function(e) {
    var key = e.keyCode || e.which, keyString = e.key || key, i2, state, a, path;
    if (keyString === "Shift" || key === 16) {
      _SHIFT = true;
    } else if (keyString === "Control" || key === 17) {
      _CTRL = true;
    } else if (keyString === "Meta" || key === 91) {
      _CMD = true;
    } else if (keyString === "Alt" || key === 18) {
      _ALT = true;
      i2 = _selectedPaths.length;
      while (--i2 > -1) {
        _selectedPaths[i2]._onPressAlt();
      }
    } else if ((keyString === "z" || key === 90) && (_CTRL || _CMD) && _history.length > 1) {
      _history.shift();
      state = _history[0];
      if (state) {
        path = state.path;
        path.path.setAttribute("d", state.d);
        path.path.setAttribute("transform", state.transform);
        path.init();
        a = path._anchors;
        for (i2 = 0; i2 < a.length; i2++) {
          if (state.selectedIndexes.indexOf(a[i2].i) !== -1) {
            path._selectedAnchors.push(a[i2]);
          }
        }
        path._updateAnchors();
        path.update();
        if (path.vars.onUndo) {
          path.vars.onUndo.call(path);
        }
      }
    } else if (keyString === "Delete" || keyString === "Backspace" || key === 8 || key === 46 || key === 63272 || key === "d" && (_CTRL || _CMD)) {
      i2 = _selectedPaths.length;
      while (--i2 > -1) {
        _selectedPaths[i2]._deleteSelectedAnchors();
      }
    } else if ((keyString === "a" || key === 65) && (_CMD || _CTRL)) {
      i2 = _selectedPaths.length;
      while (--i2 > -1) {
        _selectedPaths[i2].select(true);
      }
    }
  }, true);
  _doc11.addEventListener("keyup", function(e) {
    var key = e.key || e.keyCode || e.which;
    if (key === "Shift" || key === 16) {
      _SHIFT = false;
    } else if (key === "Control" || key === 17) {
      _CTRL = false;
    } else if (key === "Meta" || key === 91) {
      _CMD = false;
    } else if (key === "Alt" || key === 18) {
      _ALT = false;
      var i2 = _selectedPaths.length;
      while (--i2 > -1) {
        _selectedPaths[i2]._onReleaseAlt();
      }
    }
  }, true);
  _supportsPointer2 = !!_win11.PointerEvent;
  _addListener7(_doc11, "mouseup", _checkDeselect);
  _addListener7(_doc11, "touchend", _checkDeselect);
  _addListener7(_doc11, "touchcancel", _emptyFunc9);
  _addListener7(_win11, "touchmove", _emptyFunc9);
  _body8 && _body8.addEventListener("touchstart", _emptyFunc9);
  _coreInitted22 = 1;
};
var _onPress = function _onPress2(e) {
  var self = this, ctm = getGlobalMatrix(self.target.parentNode, true), touchEventTarget, temp;
  this._matrix = this.target.transform.baseVal.getItem(0).matrix;
  this._ctm = ctm;
  if (_touchEventLookup2[e.type]) {
    touchEventTarget = e.type.indexOf("touch") !== -1 ? e.currentTarget || e.target : _doc11;
    _addListener7(touchEventTarget, "touchend", self._onRelease);
    _addListener7(touchEventTarget, "touchmove", self._onMove);
    _addListener7(touchEventTarget, "touchcancel", self._onRelease);
    _addListener7(_doc11, "touchstart", _onMultiTouchDocument3);
    _addListener7(_win11, "touchforcechange", _preventDefault3);
  } else {
    touchEventTarget = null;
    _addListener7(_doc11, "mousemove", self._onMove);
  }
  if (!_supportsPointer2) {
    _addListener7(_doc11, "mouseup", self._onRelease);
  }
  _preventDefault3(e);
  _resetSelection();
  if (e.changedTouches) {
    e = self.touch = e.changedTouches[0];
    self.touchID = e.identifier;
  } else if (e.pointerId) {
    self.touchID = e.pointerId;
  } else {
    self.touch = self.touchID = null;
  }
  self._startPointerY = self.pointerY = e.pageY;
  self._startPointerX = self.pointerX = e.pageX;
  self._startElementX = self._matrix.e;
  self._startElementY = self._matrix.f;
  if (this._ctm.a === 1 && this._ctm.b === 0 && this._ctm.c === 0 && this._ctm.d === 1) {
    this._ctm = null;
  } else {
    temp = self._startPointerX * this._ctm.a + self._startPointerY * this._ctm.c + this._ctm.e;
    self._startPointerY = self._startPointerX * this._ctm.b + self._startPointerY * this._ctm.d + this._ctm.f;
    self._startPointerX = temp;
  }
  self.isPressed = _isPressed = true;
  self.touchEventTarget = touchEventTarget;
  if (self.vars.onPress) {
    self.vars.onPress.call(self.vars.callbackScope || self, self.pointerEvent);
  }
};
var _onMove = function _onMove2(e) {
  var self = this, originalEvent = e, touches, i2;
  if (!self._enabled || _isMultiTouching2 || !self.isPressed || !e) {
    return;
  }
  self.pointerEvent = e;
  touches = e.changedTouches;
  if (touches) {
    e = touches[0];
    if (e !== self.touch && e.identifier !== self.touchID) {
      i2 = touches.length;
      while (--i2 > -1 && (e = touches[i2]).identifier !== self.touchID) {
      }
      if (i2 < 0) {
        return;
      }
    }
  } else if (e.pointerId && self.touchID && e.pointerId !== self.touchID) {
    return;
  }
  _preventDefault3(originalEvent);
  self.setPointerPosition(e.pageX, e.pageY);
  if (self.vars.onDrag) {
    self.vars.onDrag.call(self.vars.callbackScope || self, self.pointerEvent);
  }
};
var _onRelease = function _onRelease2(e, force) {
  var self = this;
  if (!self._enabled || !self.isPressed || e && self.touchID != null && !force && (e.pointerId && e.pointerId !== self.touchID || e.changedTouches && !_hasTouchID3(e.changedTouches, self.touchID))) {
    return;
  }
  _interacted();
  self.isPressed = _isPressed = false;
  var originalEvent = e, wasDragging = self.isDragging, touchEventTarget = self.touchEventTarget, touches, i2;
  if (touchEventTarget) {
    _removeListener7(touchEventTarget, "touchend", self._onRelease);
    _removeListener7(touchEventTarget, "touchmove", self._onMove);
    _removeListener7(touchEventTarget, "touchcancel", self._onRelease);
    _removeListener7(_doc11, "touchstart", _onMultiTouchDocument3);
  } else {
    _removeListener7(_doc11, "mousemove", self._onMove);
  }
  if (!_supportsPointer2) {
    _removeListener7(_doc11, "mouseup", self._onRelease);
    if (e && e.target) {
      _removeListener7(e.target, "mouseup", self._onRelease);
    }
  }
  if (wasDragging) {
    self.isDragging = false;
  } else if (self.vars.onClick) {
    self.vars.onClick.call(self.vars.callbackScope || self, originalEvent);
  }
  if (e) {
    touches = e.changedTouches;
    if (touches) {
      e = touches[0];
      if (e !== self.touch && e.identifier !== self.touchID) {
        i2 = touches.length;
        while (--i2 > -1 && (e = touches[i2]).identifier !== self.touchID) {
        }
        if (i2 < 0) {
          return;
        }
      }
    }
    self.pointerEvent = originalEvent;
    self.pointerX = e.pageX;
    self.pointerY = e.pageY;
  }
  if (originalEvent && !wasDragging && self.vars.onDragRelease) {
    self.vars.onDragRelease.call(self, self.pointerEvent);
  } else {
    if (originalEvent) {
      _preventDefault3(originalEvent);
    }
    if (self.vars.onRelease) {
      self.vars.onRelease.call(self.vars.callbackScope || self, self.pointerEvent);
    }
  }
  if (wasDragging && self.vars.onDragEnd) {
    self.vars.onDragEnd.call(self.vars.callbackScope || self, self.pointerEvent);
  }
  return true;
};
var _createSegmentAnchors = function _createSegmentAnchors2(rawPath, j, editor, vars) {
  var segment = rawPath[j], l = segment.length - (segment.closed ? 6 : 0), a = [], i2;
  for (i2 = 0; i2 < l; i2 += 6) {
    a.push(new Anchor(editor, rawPath, j, i2, vars));
  }
  segment.closed && (a[0].isClosedStart = true);
  return a;
};
var _getLength3 = function _getLength4(segment, i2, i22) {
  var x = segment[i22] - segment[i2], y = segment[i22 + 1] - segment[i2 + 1];
  return Math.sqrt(x * x + y * y);
};
var DraggableSVG = /* @__PURE__ */ (function() {
  function DraggableSVG2(target, vars) {
    this.target = typeof target === "string" ? _doc11.querySelectorAll(target)[0] : target;
    this.vars = vars || {};
    this._onPress = _bind(_onPress, this);
    this._onMove = _bind(_onMove, this);
    this._onRelease = _bind(_onRelease, this);
    this.target.setAttribute("transform", (this.target.getAttribute("transform") || "") + " translate(0,0)");
    this._matrix = _getConsolidatedMatrix(this.target);
    this.x = this._matrix.e;
    this.y = this._matrix.f;
    this.snap = vars.snap;
    if (!isNaN(vars.maxX) || !isNaN(vars.minX)) {
      this._bounds = 1;
      this.maxX = +vars.maxX;
      this.minX = +vars.minX;
    } else {
      this._bounds = 0;
    }
    this.enabled(true);
  }
  var _proto = DraggableSVG2.prototype;
  _proto.setPointerPosition = function setPointerPosition(pointerX, pointerY) {
    var rnd = 1e3, xChange, yChange, x, y, temp;
    this.pointerX = pointerX;
    this.pointerY = pointerY;
    if (this._ctm) {
      temp = pointerX * this._ctm.a + pointerY * this._ctm.c + this._ctm.e;
      pointerY = pointerX * this._ctm.b + pointerY * this._ctm.d + this._ctm.f;
      pointerX = temp;
    }
    yChange = pointerY - this._startPointerY;
    xChange = pointerX - this._startPointerX;
    if (yChange < _minimumMovement && yChange > -_minimumMovement) {
      yChange = 0;
    }
    if (xChange < _minimumMovement && xChange > -_minimumMovement) {
      xChange = 0;
    }
    x = ((this._startElementX + xChange) * rnd | 0) / rnd;
    y = ((this._startElementY + yChange) * rnd | 0) / rnd;
    if (this.snap && !_SHIFT) {
      _point.x = x;
      _point.y = y;
      this.snap.call(this, _point);
      x = _point.x;
      y = _point.y;
    }
    if (this.x !== x || this.y !== y) {
      this._matrix.f = this.y = y;
      this._matrix.e = this.x = x;
      if (!this.isDragging && this.isPressed) {
        this.isDragging = true;
        _callback5("onDragStart", this, this.pointerEvent);
      }
    }
  };
  _proto.enabled = function enabled(_enabled2) {
    if (!arguments.length) {
      return this._enabled;
    }
    var dragging;
    this._enabled = _enabled2;
    if (_enabled2) {
      if (!_supportsPointer2) {
        _addListener7(this.target, "mousedown", this._onPress);
      }
      _addListener7(this.target, "touchstart", this._onPress);
      _addListener7(this.target, "click", this._onClick, true);
    } else {
      dragging = this.isDragging;
      _removeListener7(this.target, "mousedown", this._onPress);
      _removeListener7(this.target, "touchstart", this._onPress);
      _removeListener7(_win11, "touchforcechange", _preventDefault3);
      _removeListener7(this.target, "click", this._onClick);
      if (this.touchEventTarget) {
        _removeListener7(this.touchEventTarget, "touchcancel", this._onRelease);
        _removeListener7(this.touchEventTarget, "touchend", this._onRelease);
        _removeListener7(this.touchEventTarget, "touchmove", this._onMove);
      }
      _removeListener7(_doc11, "mouseup", this._onRelease);
      _removeListener7(_doc11, "mousemove", this._onMove);
      this.isDragging = this.isPressed = false;
      if (dragging) {
        _callback5("onDragEnd", this, this.pointerEvent);
      }
    }
    return this;
  };
  _proto.endDrag = function endDrag(e) {
    this._onRelease(e);
  };
  return DraggableSVG2;
})();
var Anchor = /* @__PURE__ */ (function() {
  function Anchor2(editor, rawPath, j, i2, vars) {
    this.editor = editor;
    this.element = _createSVG("path", editor._selection, {
      fill: _selectionColor,
      stroke: _selectionColor,
      strokeWidth: 2,
      vectorEffect: "non-scaling-stroke"
    });
    this.update(rawPath, j, i2);
    this.element._gsSelection = true;
    this.vars = vars || {};
    this._draggable = new DraggableSVG(this.element, {
      callbackScope: this,
      onDrag: this.onDrag,
      snap: this.vars.snap,
      onPress: this.onPress,
      onRelease: this.onRelease,
      onClick: this.onClick,
      onDragEnd: this.onDragEnd
    });
  }
  var _proto2 = Anchor2.prototype;
  _proto2.onPress = function onPress() {
    _callback5("onPress", this);
  };
  _proto2.onClick = function onClick() {
    _callback5("onClick", this);
  };
  _proto2.onDrag = function onDrag() {
    var s = this.segment;
    this.vars.onDrag.call(this.vars.callbackScope || this, this, this._draggable.x - s[this.i], this._draggable.y - s[this.i + 1]);
  };
  _proto2.onDragEnd = function onDragEnd() {
    _callback5("onDragEnd", this);
  };
  _proto2.onRelease = function onRelease() {
    _callback5("onRelease", this);
  };
  _proto2.update = function update(rawPath, j, i2) {
    if (rawPath) {
      this.rawPath = rawPath;
    }
    if (arguments.length <= 1) {
      j = this.j;
      i2 = this.i;
    } else {
      this.j = j;
      this.i = i2;
    }
    var prevSmooth = this.smooth, segment = this.rawPath[j], pi = i2 === 0 && segment.closed ? segment.length - 4 : i2 - 2;
    this.segment = segment;
    this.smooth = i2 > 0 && i2 < segment.length - 2 && Math.abs(Math.atan2(segment[i2 + 1] - segment[pi + 1], segment[i2] - segment[pi]) - Math.atan2(segment[i2 + 3] - segment[i2 + 1], segment[i2 + 2] - segment[i2])) < 0.09 ? 2 : 0;
    if (this.smooth !== prevSmooth) {
      this.element.setAttribute("d", this.smooth ? this.editor._circleHandle : this.editor._squareHandle);
    }
    this.element.setAttribute("transform", "translate(" + segment[i2] + "," + segment[i2 + 1] + ")");
  };
  return Anchor2;
})();
var PathEditor = /* @__PURE__ */ (function() {
  function PathEditor2(target, vars) {
    vars = vars || {};
    _coreInitted22 || _initCore37();
    this.vars = vars;
    this.path = typeof target === "string" ? _doc11.querySelectorAll(target)[0] : target;
    this._g = _createSVG("g", this.path.ownerSVGElement, {
      "class": "path-editor-g path-editor"
    });
    this._selectionHittest = _createSVG("path", this._g, {
      stroke: "transparent",
      strokeWidth: 16,
      fill: "none",
      vectorEffect: "non-scaling-stroke"
    });
    this._selection = vars._selection || _createSVG("g", this._g, {
      "class": "path-editor-selection path-editor"
    });
    this._selectionPath = _createSVG("path", this._selection, {
      stroke: _selectionColor,
      strokeWidth: 2,
      fill: "none",
      vectorEffect: "non-scaling-stroke"
    });
    this._selectedAnchors = [];
    this._line1 = _createSVG("polyline", this._selection, {
      stroke: _selectionColor,
      strokeWidth: 2,
      vectorEffect: "non-scaling-stroke"
    });
    this._line2 = _createSVG("polyline", this._selection, {
      stroke: _selectionColor,
      strokeWidth: 2,
      vectorEffect: "non-scaling-stroke"
    });
    this._line1.style.pointerEvents = this._line2.style.pointerEvents = this._selectionPath.style.pointerEvents = "none";
    this._enabled = true;
    var ctm = this.path.parentNode.getScreenCTM().inverse(), size = (ctm.a + ctm.d) / 2 * (vars.handleSize || 5);
    this._squareHandle = _getSquarePathData(size);
    this._circleHandle = _getCirclePathData(size * 1.15);
    this._handle1 = _createSVG("path", this._selection, {
      d: this._squareHandle,
      fill: _selectionColor,
      stroke: "transparent",
      strokeWidth: 6
    });
    this._handle2 = _createSVG("path", this._selection, {
      d: this._squareHandle,
      fill: _selectionColor,
      stroke: "transparent",
      strokeWidth: 6
    });
    this._handle1._draggable = new DraggableSVG(this._handle1, {
      onDrag: this._onDragHandle1,
      callbackScope: this,
      onPress: this._onPressHandle1,
      onRelease: this._onReleaseHandle,
      onClick: this._onClickHandle1,
      snap: vars.handleSnap
    });
    this._handle2._draggable = new DraggableSVG(this._handle2, {
      onDrag: this._onDragHandle2,
      callbackScope: this,
      onPress: this._onPressHandle2,
      onRelease: this._onReleaseHandle,
      onClick: this._onClickHandle2,
      snap: vars.handleSnap
    });
    this._handle1.style.visibility = this._handle2.style.visibility = "hidden";
    var selectionItems = [this._handle1, this._handle2, this._line1, this._line2, this._selection, this._selectionPath, this._selectionHittest], i2 = selectionItems.length;
    while (--i2 > -1) {
      selectionItems[i2]._gsSelection = true;
    }
    if (vars.draggable !== false) {
      this._draggable = new DraggableSVG(this._selectionHittest, {
        callbackScope: this,
        onPress: this.select,
        onRelease: this._onRelease,
        onDrag: this._onDragPath,
        onDragEnd: this._saveState,
        maxX: this.vars.maxX,
        minX: this.vars.minX
      });
    }
    this.init();
    this._selection.style.visibility = vars.selected === false ? "hidden" : "visible";
    if (vars.selected !== false) {
      this.path._gsSelection = true;
      _selectedPaths.push(this);
    }
    this._saveState();
    if (!_supportsPointer2) {
      _addListener7(this._selectionHittest, "mousedown", _bind(this._onClickSelectionPath, this));
      _addListener7(this._selectionHittest, "mouseup", _bind(this._onRelease, this));
    }
    _addListener7(this._selectionHittest, "touchstart", _bind(this._onClickSelectionPath, this));
    _addListener7(this._selectionHittest, "touchend", _bind(this._onRelease, this));
    _context7(this);
  }
  var _proto3 = PathEditor2.prototype;
  _proto3._onRelease = function _onRelease3(e) {
    var anchor = this._editingAnchor;
    if (anchor) {
      _editingAxis.x = anchor.segment[anchor.i];
      _editingAxis.y = anchor.segment[anchor.i + 1];
    }
    _removeListener7(_win11, "touchforcechange", _preventDefault3);
    _callback5("onRelease", this, e);
  };
  _proto3.init = function init16() {
    var pathData = this.path.getAttribute("d"), rawPath = stringToRawPath(pathData), transform = this.path.getAttribute("transform") || "translate(0,0)", createAnchors = !this._rawPath || rawPath.totalPoints !== this._rawPath.totalPoints || rawPath.map(function(s) {
      return s.length;
    }).join(",") !== this._rawPath.map(function(s) {
      return s.length;
    }).join(","), anchorVars = {
      callbackScope: this,
      snap: this.vars.anchorSnap,
      onDrag: this._onDragAnchor,
      onPress: this._onPressAnchor,
      onRelease: this._onRelease,
      onClick: this._onClickAnchor,
      onDragEnd: this._onDragEndAnchor,
      maxX: this.vars.maxX,
      minX: this.vars.minX
    }, l, i2;
    if (createAnchors && this._anchors && this._anchors.length) {
      for (i2 = 0; i2 < this._anchors.length; i2++) {
        this._anchors[i2].element.parentNode.removeChild(this._anchors[i2].element);
        this._anchors[i2]._draggable.enabled(false);
      }
      this._selectedAnchors.length = 0;
    }
    this._rawPath = rawPath;
    if (createAnchors) {
      this._anchors = _createSegmentAnchors(rawPath, 0, this, anchorVars);
      l = rawPath.length;
      if (l > 1) {
        for (i2 = 1; i2 < l; i2++) {
          this._anchors = this._anchors.concat(_createSegmentAnchors(rawPath, i2, this, anchorVars));
        }
      }
    } else {
      i2 = this._anchors.length;
      while (--i2 > -1) {
        this._anchors[i2].update(rawPath);
      }
    }
    this._selection.appendChild(this._handle1);
    this._selection.appendChild(this._handle2);
    this._selectionPath.setAttribute("d", pathData);
    this._selectionHittest.setAttribute("d", pathData);
    this._g.setAttribute("transform", _getConcatenatedTransforms(this.path.parentNode) || "translate(0,0)");
    this._selection.setAttribute("transform", transform);
    this._selectionHittest.setAttribute("transform", transform);
    this._updateAnchors();
    return this;
  };
  _proto3._saveState = function _saveState() {
    _addHistory(this);
  };
  _proto3._onClickSelectionPath = function _onClickSelectionPath(e) {
    if (this._selection.style.visibility === "hidden") {
      this.select();
    } else if (_ALT || e && e.altKey) {
      var anchorVars = {
        callbackScope: this,
        snap: this.vars.anchorSnap,
        onDrag: this._onDragAnchor,
        onPress: this._onPressAnchor,
        onRelease: this._onRelease,
        onClick: this._onClickAnchor,
        onDragEnd: this._onDragEndAnchor,
        maxX: this.vars.maxX,
        minX: this.vars.minX
      }, ctm = this._selection.getScreenCTM().inverse(), newIndex, _i2, anchor, x, y, closestData;
      if (this._draggable) {
        this._draggable._onRelease(e);
      }
      if (ctm) {
        x = e.clientX * ctm.a + e.clientY * ctm.c + ctm.e;
        y = e.clientX * ctm.b + e.clientY * ctm.d + ctm.f;
      }
      closestData = getClosestData(this._rawPath, x, y);
      subdivideSegment(this._rawPath[closestData.j], closestData.i, closestData.t);
      newIndex = closestData.i + 6;
      for (_i2 = 0; _i2 < this._anchors.length; _i2++) {
        if (this._anchors[_i2].i >= newIndex && this._anchors[_i2].j === closestData.j) {
          this._anchors[_i2].i += 6;
        }
      }
      anchor = new Anchor(this, this._rawPath, closestData.j, newIndex, anchorVars);
      this._selection.appendChild(this._handle1);
      this._selection.appendChild(this._handle2);
      anchor._draggable._onPress(e);
      _recentlyAddedAnchor = anchor;
      this._anchors.push(anchor);
      this._selectedAnchors.length = 0;
      this._selectedAnchors.push(anchor);
      this._updateAnchors();
      this.update();
      this._saveState();
    }
    _resetSelection();
    _addListener7(_win11, "touchforcechange", _preventDefault3);
    _callback5("onPress", this);
  };
  _proto3._onClickHandle1 = function _onClickHandle1() {
    var anchor = this._editingAnchor, i2 = anchor.i, s = anchor.segment, pi = anchor.isClosedStart ? s.length - 4 : i2 - 2;
    if (_ALT && Math.abs(s[i2] - s[pi]) < 5 && Math.abs(s[i2 + 1] - s[pi + 1]) < 5) {
      this._onClickAnchor(anchor);
    }
  };
  _proto3._onClickHandle2 = function _onClickHandle2() {
    var anchor = this._editingAnchor, i2 = anchor.i, s = anchor.segment;
    if (_ALT && Math.abs(s[i2] - s[i2 + 2]) < 5 && Math.abs(s[i2 + 1] - s[i2 + 3]) < 5) {
      this._onClickAnchor(anchor);
    }
  };
  _proto3._onDragEndAnchor = function _onDragEndAnchor(e) {
    _recentlyAddedAnchor = null;
    this._saveState();
  };
  _proto3.isSelected = function isSelected() {
    return this._selectedAnchors.length > 0 || this._selection.style.visibility === "visible";
  };
  _proto3.select = function select(allAnchors) {
    this._selection.style.visibility = "visible";
    this._editingAnchor = null;
    this.path._gsSelection = true;
    if (allAnchors === true) {
      var _i2 = this._anchors.length;
      while (--_i2 > -1) {
        this._selectedAnchors[_i2] = this._anchors[_i2];
      }
    }
    if (_selectedPaths.indexOf(this) === -1) {
      _selectedPaths.push(this);
    }
    this._updateAnchors();
    return this;
  };
  _proto3.deselect = function deselect() {
    this._selection.style.visibility = "hidden";
    this._selectedAnchors.length = 0;
    this._editingAnchor = null;
    this.path._gsSelection = false;
    _selectedPaths.splice(_selectedPaths.indexOf(this), 1);
    this._updateAnchors();
    return this;
  };
  _proto3._onDragPath = function _onDragPath(e) {
    var transform = this._selectionHittest.getAttribute("transform") || "translate(0,0)";
    this._selection.setAttribute("transform", transform);
    this.path.setAttribute("transform", transform);
  };
  _proto3._onPressAnchor = function _onPressAnchor(anchor) {
    if (this._selectedAnchors.indexOf(anchor) === -1) {
      if (!_SHIFT) {
        this._selectedAnchors.length = 0;
      }
      this._selectedAnchors.push(anchor);
    } else if (_SHIFT) {
      this._selectedAnchors.splice(this._selectedAnchors.indexOf(anchor), 1);
      anchor._draggable.endDrag();
    }
    _editingAxis.x = anchor.segment[anchor.i];
    _editingAxis.y = anchor.segment[anchor.i + 1];
    this._updateAnchors();
    _callback5("onPress", this);
  };
  _proto3._deleteSelectedAnchors = function _deleteSelectedAnchors() {
    var anchors = this._selectedAnchors, i2 = anchors.length, anchor, index, j, jIndex;
    while (--i2 > -1) {
      anchor = anchors[i2];
      anchor.element.parentNode.removeChild(anchor.element);
      anchor._draggable.enabled(false);
      index = anchor.i;
      jIndex = anchor.j;
      if (!index) {
        anchor.segment.splice(index, 6);
      } else if (index < anchor.segment.length - 2) {
        anchor.segment.splice(index - 2, 6);
      } else {
        anchor.segment.splice(index - 4, 6);
      }
      anchors.splice(i2, 1);
      this._anchors.splice(this._anchors.indexOf(anchor), 1);
      for (j = 0; j < this._anchors.length; j++) {
        if (this._anchors[j].i >= index && this._anchors[j].j === jIndex) {
          this._anchors[j].i -= 6;
        }
      }
    }
    this._updateAnchors();
    this.update();
    this._saveState();
    if (this.vars.onDeleteAnchor) {
      this.vars.onDeleteAnchor.call(this.vars.callbackScope || this);
    }
  };
  _proto3._onClickAnchor = function _onClickAnchor(anchor) {
    var i2 = anchor.i, segment = anchor.segment, pi = anchor.isClosedStart ? segment.length - 4 : i2 - 2, rnd = 1e3, isEnd = !i2 || i2 >= segment.length - 2, angle1, angle2, length1, length2, sin, cos;
    if (_ALT && _recentlyAddedAnchor !== anchor && this._editingAnchor) {
      anchor.smooth = !anchor.smooth;
      if (isEnd && !anchor.isClosedStart) {
        anchor.smooth = false;
      }
      anchor.element.setAttribute("d", anchor.smooth ? this._circleHandle : this._squareHandle);
      if (anchor.smooth && (!isEnd || anchor.isClosedStart)) {
        angle1 = Math.atan2(segment[i2 + 1] - segment[pi + 1], segment[i2] - segment[pi]);
        angle2 = Math.atan2(segment[i2 + 3] - segment[i2 + 1], segment[i2 + 2] - segment[i2]);
        angle1 = (angle1 + angle2) / 2;
        length1 = _getLength3(segment, pi, i2);
        length2 = _getLength3(segment, i2, i2 + 2);
        if (length1 < 0.2) {
          length1 = _getLength3(segment, i2, pi - 4) / 4;
          angle1 = angle2 || Math.atan2(segment[i2 + 7] - segment[pi - 3], segment[i2 + 6] - segment[pi - 4]);
        }
        if (length2 < 0.2) {
          length2 = _getLength3(segment, i2, i2 + 6) / 4;
          angle2 = angle1 || Math.atan2(segment[i2 + 7] - segment[pi - 3], segment[i2 + 6] - segment[pi - 4]);
        }
        sin = Math.sin(angle1);
        cos = Math.cos(angle1);
        if (Math.abs(angle2 - angle1) < Math.PI / 2) {
          sin = -sin;
          cos = -cos;
        }
        segment[pi] = ((segment[i2] + cos * length1) * rnd | 0) / rnd;
        segment[pi + 1] = ((segment[i2 + 1] + sin * length1) * rnd | 0) / rnd;
        segment[i2 + 2] = ((segment[i2] - cos * length2) * rnd | 0) / rnd;
        segment[i2 + 3] = ((segment[i2 + 1] - sin * length2) * rnd | 0) / rnd;
        this._updateAnchors();
        this.update();
        this._saveState();
      } else if (!anchor.smooth && (!isEnd || anchor.isClosedStart)) {
        if (i2 || anchor.isClosedStart) {
          segment[pi] = segment[i2];
          segment[pi + 1] = segment[i2 + 1];
        }
        if (i2 < segment.length - 2) {
          segment[i2 + 2] = segment[i2];
          segment[i2 + 3] = segment[i2 + 1];
        }
        this._updateAnchors();
        this.update();
        this._saveState();
      }
    } else if (!_SHIFT) {
      this._selectedAnchors.length = 0;
      this._selectedAnchors.push(anchor);
    }
    _recentlyAddedAnchor = null;
    this._updateAnchors();
  };
  _proto3._updateAnchors = function _updateAnchors() {
    var anchor = this._selectedAnchors.length === 1 ? this._selectedAnchors[0] : null, segment = anchor ? anchor.segment : null, i2, x, y;
    this._editingAnchor = anchor;
    for (i2 = 0; i2 < this._anchors.length; i2++) {
      this._anchors[i2].element.style.fill = this._selectedAnchors.indexOf(this._anchors[i2]) !== -1 ? _selectionColor : "white";
    }
    if (anchor) {
      this._handle1.setAttribute("d", anchor.smooth ? this._circleHandle : this._squareHandle);
      this._handle2.setAttribute("d", anchor.smooth ? this._circleHandle : this._squareHandle);
    }
    i2 = anchor ? anchor.i : 0;
    if (anchor && (i2 || anchor.isClosedStart)) {
      x = anchor.isClosedStart ? segment[segment.length - 4] : segment[i2 - 2];
      y = anchor.isClosedStart ? segment[segment.length - 3] : segment[i2 - 1];
      this._handle1.style.visibility = this._line1.style.visibility = !_ALT && x === segment[i2] && y === segment[i2 + 1] ? "hidden" : "visible";
      this._handle1.setAttribute("transform", "translate(" + x + _comma + y + ")");
      this._line1.setAttribute("points", x + _comma + y + _comma + segment[i2] + _comma + segment[i2 + 1]);
    } else {
      this._handle1.style.visibility = this._line1.style.visibility = "hidden";
    }
    if (anchor && i2 < segment.length - 2) {
      x = segment[i2 + 2];
      y = segment[i2 + 3];
      this._handle2.style.visibility = this._line2.style.visibility = !_ALT && x === segment[i2] && y === segment[i2 + 1] ? "hidden" : "visible";
      this._handle2.setAttribute("transform", "translate(" + x + _comma + y + ")");
      this._line2.setAttribute("points", segment[i2] + _comma + segment[i2 + 1] + _comma + x + _comma + y);
    } else {
      this._handle2.style.visibility = this._line2.style.visibility = "hidden";
    }
  };
  _proto3._onPressAlt = function _onPressAlt() {
    var anchor = this._editingAnchor;
    if (anchor) {
      if (anchor.i || anchor.isClosedStart) {
        this._handle1.style.visibility = this._line1.style.visibility = "visible";
      }
      if (anchor.i < anchor.segment.length - 2) {
        this._handle2.style.visibility = this._line2.style.visibility = "visible";
      }
    }
  };
  _proto3._onReleaseAlt = function _onReleaseAlt() {
    var anchor = this._editingAnchor, s, i2, pi;
    if (anchor) {
      s = anchor.segment;
      i2 = anchor.i;
      pi = anchor.isClosedStart ? s.length - 4 : i2 - 2;
      if (s[i2] === s[pi] && s[i2 + 1] === s[pi + 1]) {
        this._handle1.style.visibility = this._line1.style.visibility = "hidden";
      }
      if (s[i2] === s[i2 + 2] && s[i2 + 1] === s[i2 + 3]) {
        this._handle2.style.visibility = this._line2.style.visibility = "hidden";
      }
    }
  };
  _proto3._onPressHandle1 = function _onPressHandle1() {
    if (this._editingAnchor.smooth) {
      this._oppositeHandleLength = _getLength3(this._editingAnchor.segment, this._editingAnchor.i, this._editingAnchor.i + 2);
    }
    _callback5("onPress", this);
  };
  _proto3._onPressHandle2 = function _onPressHandle2() {
    if (this._editingAnchor.smooth) {
      this._oppositeHandleLength = _getLength3(this._editingAnchor.segment, this._editingAnchor.isClosedStart ? this._editingAnchor.segment.length - 4 : this._editingAnchor.i - 2, this._editingAnchor.i);
    }
    _callback5("onPress", this);
  };
  _proto3._onReleaseHandle = function _onReleaseHandle(e) {
    this._onRelease(e);
    this._saveState();
  };
  _proto3._onDragHandle1 = function _onDragHandle1() {
    var anchor = this._editingAnchor, s = anchor.segment, i2 = anchor.i, pi = anchor.isClosedStart ? s.length - 4 : i2 - 2, rnd = 1e3, x = this._handle1._draggable.x, y = this._handle1._draggable.y, angle;
    s[pi] = x = (x * rnd | 0) / rnd;
    s[pi + 1] = y = (y * rnd | 0) / rnd;
    if (anchor.smooth) {
      if (_ALT) {
        anchor.smooth = false;
        anchor.element.setAttribute("d", this._squareHandle);
        this._handle1.setAttribute("d", this._squareHandle);
        this._handle2.setAttribute("d", this._squareHandle);
      } else {
        angle = Math.atan2(s[i2 + 1] - y, s[i2] - x);
        x = this._oppositeHandleLength * Math.cos(angle);
        y = this._oppositeHandleLength * Math.sin(angle);
        s[i2 + 2] = ((s[i2] + x) * rnd | 0) / rnd;
        s[i2 + 3] = ((s[i2 + 1] + y) * rnd | 0) / rnd;
      }
    }
    this.update();
  };
  _proto3._onDragHandle2 = function _onDragHandle2() {
    var anchor = this._editingAnchor, s = anchor.segment, i2 = anchor.i, pi = anchor.isClosedStart ? s.length - 4 : i2 - 2, rnd = 1e3, x = this._handle2._draggable.x, y = this._handle2._draggable.y, angle;
    s[i2 + 2] = x = (x * rnd | 0) / rnd;
    s[i2 + 3] = y = (y * rnd | 0) / rnd;
    if (anchor.smooth) {
      if (_ALT) {
        anchor.smooth = false;
        anchor.element.setAttribute("d", this._squareHandle);
        this._handle1.setAttribute("d", this._squareHandle);
        this._handle2.setAttribute("d", this._squareHandle);
      } else {
        angle = Math.atan2(s[i2 + 1] - y, s[i2] - x);
        x = this._oppositeHandleLength * Math.cos(angle);
        y = this._oppositeHandleLength * Math.sin(angle);
        s[pi] = ((s[i2] + x) * rnd | 0) / rnd;
        s[pi + 1] = ((s[i2 + 1] + y) * rnd | 0) / rnd;
      }
    }
    this.update();
  };
  _proto3._onDragAnchor = function _onDragAnchor(anchor, changeX, changeY) {
    var anchors = this._selectedAnchors, l = anchors.length, rnd = 1e3, i2, j, s, a, pi;
    for (j = 0; j < l; j++) {
      a = anchors[j];
      i2 = a.i;
      s = a.segment;
      if (i2) {
        s[i2 - 2] = ((s[i2 - 2] + changeX) * rnd | 0) / rnd;
        s[i2 - 1] = ((s[i2 - 1] + changeY) * rnd | 0) / rnd;
      } else if (a.isClosedStart) {
        pi = s.length - 2;
        s[pi] = _round27(s[pi] + changeX);
        s[pi + 1] = _round27(s[pi + 1] + changeY);
        s[pi - 2] = _round27(s[pi - 2] + changeX);
        s[pi - 1] = _round27(s[pi - 1] + changeY);
      }
      s[i2] = ((s[i2] + changeX) * rnd | 0) / rnd;
      s[i2 + 1] = ((s[i2 + 1] + changeY) * rnd | 0) / rnd;
      if (i2 < s.length - 2) {
        s[i2 + 2] = ((s[i2 + 2] + changeX) * rnd | 0) / rnd;
        s[i2 + 3] = ((s[i2 + 3] + changeY) * rnd | 0) / rnd;
      }
      if (a !== anchor) {
        a.element.setAttribute("transform", "translate(" + s[i2] + _comma + s[i2 + 1] + ")");
      }
    }
    this.update();
  };
  _proto3.enabled = function enabled(_enabled2) {
    if (!arguments.length) {
      return this._enabled;
    }
    var i2 = this._anchors.length;
    while (--i2 > -1) {
      this._anchors[i2]._draggable.enabled(_enabled2);
    }
    this._enabled = _enabled2;
    this._handle1._draggable.enabled(_enabled2);
    this._handle2._draggable.enabled(_enabled2);
    if (this._draggable) {
      this._draggable.enabled(_enabled2);
    }
    if (!_enabled2) {
      this.deselect();
      this._selectionHittest.parentNode && this._selectionHittest.parentNode.removeChild(this._selectionHittest);
      this._selection.parentNode && this._selection.parentNode.removeChild(this._selection);
    } else if (!this._selection.parentNode) {
      this.path.ownerSVGElement.appendChild(this._selectionHittest);
      this.path.ownerSVGElement.appendChild(this._selection);
      this.init();
      this._saveState();
    }
    this._updateAnchors();
    return this.update();
  };
  _proto3.update = function update(readPath) {
    var d = "", anchor = this._editingAnchor, i2, s, x, y, pi;
    if (readPath) {
      this.init();
    }
    if (anchor) {
      i2 = anchor.i;
      s = anchor.segment;
      if (i2 || anchor.isClosedStart) {
        pi = anchor.isClosedStart ? s.length - 4 : i2 - 2;
        x = s[pi];
        y = s[pi + 1];
        this._handle1.setAttribute("transform", "translate(" + x + _comma + y + ")");
        this._line1.setAttribute("points", x + _comma + y + _comma + s[i2] + _comma + s[i2 + 1]);
      }
      if (i2 < s.length - 2) {
        x = s[i2 + 2];
        y = s[i2 + 3];
        this._handle2.setAttribute("transform", "translate(" + x + _comma + y + ")");
        this._line2.setAttribute("points", s[i2] + _comma + s[i2 + 1] + _comma + x + _comma + y);
      }
    }
    if (readPath) {
      d = this.path.getAttribute("d");
    } else {
      for (i2 = 0; i2 < this._rawPath.length; i2++) {
        s = this._rawPath[i2];
        if (s.length > 7) {
          d += "M" + s[0] + _comma + s[1] + "C" + s.slice(2).join(_comma);
        }
      }
      this.path.setAttribute("d", d);
      this._selectionPath.setAttribute("d", d);
      this._selectionHittest.setAttribute("d", d);
    }
    if (this.vars.onUpdate && this._enabled) {
      _callback5("onUpdate", this, d);
    }
    return this;
  };
  _proto3.getRawPath = function getRawPath2(applyTransforms, offsetX, offsetY) {
    if (applyTransforms) {
      var m = _getConsolidatedMatrix(this.path);
      return transformRawPath(copyRawPath(this._rawPath), 1, 0, 0, 1, m.e + (offsetX || 0), m.f + (offsetY || 0));
    }
    return this._rawPath;
  };
  _proto3.getString = function getString(applyTransforms, offsetX, offsetY) {
    if (applyTransforms) {
      var m = _getConsolidatedMatrix(this.path);
      return rawPathToString(transformRawPath(copyRawPath(this._rawPath), 1, 0, 0, 1, m.e + (offsetX || 0), m.f + (offsetY || 0)));
    }
    return this.path.getAttribute("d");
  };
  _proto3.getNormalizedSVG = function getNormalizedSVG(height, originY, shorten, onEaseError) {
    var s = this._rawPath[0], tx = s[0] * -1, ty = originY === 0 ? 0 : -(originY || s[1]), l = s.length, sx = 1 / (s[l - 2] + tx), sy = -height || s[l - 1] + ty, rnd = 1e3, points, i2, x1, y1, x2, y2;
    _temp3.length = 0;
    if (sy) {
      sy = 1 / sy;
    } else {
      sy = -sx;
    }
    sx *= rnd;
    sy *= rnd;
    for (i2 = 0; i2 < l; i2 += 2) {
      _temp3[i2] = ((s[i2] + tx) * sx | 0) / rnd;
      _temp3[i2 + 1] = ((s[i2 + 1] + ty) * sy | 0) / rnd;
    }
    if (onEaseError) {
      points = [];
      l = _temp3.length;
      for (i2 = 2; i2 < l; i2 += 6) {
        x1 = _temp3[i2 - 2];
        y1 = _temp3[i2 - 1];
        x2 = _temp3[i2 + 4];
        y2 = _temp3[i2 + 5];
        points.push(x1, y1, x2, y2);
        bezierToPoints(x1, y1, _temp3[i2], _temp3[i2 + 1], _temp3[i2 + 2], _temp3[i2 + 3], x2, y2, 1e-3, points, points.length - 2);
      }
      x1 = points[0];
      l = points.length;
      for (i2 = 2; i2 < l; i2 += 2) {
        x2 = points[i2];
        if (x2 < x1 || x2 > 1 || x2 < 0) {
          onEaseError();
          break;
        }
        x1 = x2;
      }
    }
    if (shorten && l === 8 && _temp3[0] === 0 && _temp3[1] === 0 && _temp3[l - 2] === 1 && _temp3[l - 1] === 1) {
      return _temp3.slice(2, 6).join(",");
    }
    _temp3[2] = "C" + _temp3[2];
    return "M" + _temp3.join(",");
  };
  _proto3.kill = function kill5() {
    this.enabled(false);
    this._g.parentNode && this._g.parentNode.removeChild(this._g);
  };
  _proto3.revert = function revert() {
    this.kill();
  };
  return PathEditor2;
})();
PathEditor.simplifyPoints = simplifyPoints;
PathEditor.pointsToSegment = pointsToSegment;
PathEditor.simplifySVG = function(data, vars) {
  var element, points, i2, x1, x2, y1, y2, bezier, precision, tolerance, l, cornerThreshold;
  vars = vars || {};
  tolerance = vars.tolerance || 1;
  precision = vars.precision || 1 / tolerance;
  cornerThreshold = (vars.cornerThreshold === void 0 ? 18 : +vars.cornerThreshold) * _DEG2RAD7;
  if (typeof data !== "string") {
    element = data;
    data = element.getAttribute("d");
  }
  if (data.charAt(0) === "#" || data.charAt(0) === ".") {
    element = _doc11.querySelector(data);
    if (element) {
      data = element.getAttribute("d");
    }
  }
  points = vars.curved === false && !/[achqstvz]/ig.test(data) ? data.match(_numbersExp2) : stringToRawPath(data)[0];
  if (vars.curved !== false) {
    bezier = points;
    points = [];
    l = bezier.length;
    for (i2 = 2; i2 < l; i2 += 6) {
      x1 = +bezier[i2 - 2];
      y1 = +bezier[i2 - 1];
      x2 = +bezier[i2 + 4];
      y2 = +bezier[i2 + 5];
      points.push(_round27(x1), _round27(y1), _round27(x2), _round27(y2));
      bezierToPoints(x1, y1, +bezier[i2], +bezier[i2 + 1], +bezier[i2 + 2], +bezier[i2 + 3], x2, y2, 1 / (precision * 2e5), points, points.length - 2);
    }
    points = pointsToSegment(simplifyPoints(points, tolerance), vars.curviness, cornerThreshold);
    points[2] = "C" + points[2];
  } else {
    points = simplifyPoints(points, tolerance);
  }
  data = "M" + points.join(",");
  if (element) {
    element.setAttribute("d", data);
  }
  return data;
};
PathEditor.create = function(target, vars) {
  return new PathEditor(target, vars);
};
PathEditor.editingAxis = _editingAxis;
PathEditor.getSnapFunction = function(vars) {
  var r = vars.radius || 2, big = 1e20, minX = vars.x || vars.x === 0 ? vars.x : vars.width ? 0 : -big, minY = vars.y || vars.y === 0 ? vars.y : vars.height ? 0 : -big, maxX = minX + (vars.width || big * big), maxY = minY + (vars.height || big * big), containX = vars.containX !== false, containY = vars.containY !== false, axis = vars.axis, grid = vars.gridSize;
  r *= r;
  return function(p2) {
    var x = p2.x, y = p2.y, gridX, gridY, dx, dy;
    if (containX && x < minX || (dx = x - minX) * dx < r) {
      x = minX;
    } else if (containX && x > maxX || (dx = maxX - x) * dx < r) {
      x = maxX;
    }
    if (containY && y < minY || (dy = y - minY) * dy < r) {
      y = minY;
    } else if (containY && y > maxY || (dy = maxY - y) * dy < r) {
      y = maxY;
    }
    if (axis) {
      dx = x - axis.x;
      dy = y - axis.y;
      if (dx * dx < r) {
        x = axis.x;
      }
      if (dy * dy < r) {
        y = axis.y;
      }
    }
    if (grid) {
      gridX = minX + Math.round((x - minX) / grid) * grid;
      dx = gridX - x;
      gridY = minY + Math.round((y - minY) / grid) * grid;
      dy = gridY - y;
      if (dx * dx + dy * dy < r) {
        x = gridX;
        y = gridY;
      }
    }
    p2.x = x;
    p2.y = y;
  };
};
PathEditor.version = "3.15.0";
PathEditor.register = _initCore37;

// examples/figma-launch/node_modules/gsap/MotionPathHelper.js
var gsap26;
var _win12;
var _doc12;
var _docEl5;
var _body9;
var MotionPathPlugin2;
var _arrayToRawPath;
var _rawPathToString;
var _context8;
var _selectorExp3 = /(^[#\.][a-z]|[a-y][a-z])/i;
var _isString13 = function _isString14(value) {
  return typeof value === "string";
};
var _createElement7 = function _createElement8(type, ns) {
  var e = _doc12.createElementNS ? _doc12.createElementNS((ns || "http://www.w3.org/1999/xhtml").replace(/^https/, "http"), type) : _doc12.createElement(type);
  return e.style ? e : _doc12.createElement(type);
};
var _getPositionOnPage = function _getPositionOnPage2(target) {
  var bounds = target.getBoundingClientRect(), windowOffsetY = _docEl5.clientTop - (_win12.pageYOffset || _docEl5.scrollTop || _body9.scrollTop || 0), windowOffsetX = _docEl5.clientLeft - (_win12.pageXOffset || _docEl5.scrollLeft || _body9.scrollLeft || 0);
  return {
    left: bounds.left + windowOffsetX,
    top: bounds.top + windowOffsetY,
    right: bounds.right + windowOffsetX,
    bottom: bounds.bottom + windowOffsetY
  };
};
var _getInitialPath = function _getInitialPath2(x, y) {
  var coordinates = [0, 31, 8, 58, 24, 75, 40, 90, 69, 100, 100, 100], i2;
  for (i2 = 0; i2 < coordinates.length; i2 += 2) {
    coordinates[i2] += x;
    coordinates[i2 + 1] += y;
  }
  return "M" + x + "," + y + "C" + coordinates.join(",");
};
var _getGlobalTime = function _getGlobalTime2(animation) {
  var time = animation.totalTime();
  while (animation) {
    time = animation.startTime() + time / (animation.timeScale() || 1);
    animation = animation.parent;
  }
  return time;
};
var _copyElement2;
var _initCopyToClipboard = function _initCopyToClipboard2() {
  _copyElement2 = _createElement7("textarea");
  _copyElement2.style.display = "none";
  _body9.appendChild(_copyElement2);
};
var _parsePath = function _parsePath2(path, target, vars) {
  return _isString13(path) && _selectorExp3.test(path) ? _doc12.querySelector(path) : Array.isArray(path) ? _rawPathToString(_arrayToRawPath([{
    x: gsap26.getProperty(target, "x"),
    y: gsap26.getProperty(target, "y")
  }].concat(path), vars)) : _isString13(path) || path && (path.tagName + "").toLowerCase() === "path" ? path : 0;
};
var _addCopyToClipboard = function _addCopyToClipboard2(target, getter, onComplete) {
  target.addEventListener("click", function(e) {
    if (e.target._gsHelper) {
      var c = getter(e.target);
      _copyElement2.value = c;
      if (c && _copyElement2.select) {
        console.log(c);
        _copyElement2.style.display = "block";
        _copyElement2.select();
        try {
          _doc12.execCommand("copy");
          _copyElement2.blur();
          onComplete && onComplete(target);
        } catch (err) {
          console.warn("Copy didn't work; this browser doesn't permit that.");
        }
        _copyElement2.style.display = "none";
      }
    }
  });
};
var _identityMatrixObject2 = {
  matrix: {
    a: 1,
    b: 0,
    c: 0,
    d: 1,
    e: 0,
    f: 0
  }
};
var _getConsolidatedMatrix3 = function _getConsolidatedMatrix4(target) {
  return (target.transform.baseVal.consolidate() || _identityMatrixObject2).matrix;
};
var _findMotionPathTween = function _findMotionPathTween2(target) {
  var tweens = gsap26.getTweensOf(target), i2 = 0;
  for (; i2 < tweens.length; i2++) {
    if (tweens[i2].vars.motionPath) {
      return tweens[i2];
    } else if (tweens[i2].timeline) {
      tweens.push.apply(tweens, tweens[i2].timeline.getChildren());
    }
  }
};
var _initCore39 = function _initCore40(core, required) {
  var message = "Please gsap.registerPlugin(MotionPathPlugin)";
  _win12 = window;
  gsap26 = gsap26 || core || _win12.gsap || console.warn(message);
  gsap26 && PathEditor.register(gsap26);
  _doc12 = document;
  _body9 = _doc12.body;
  _docEl5 = _doc12.documentElement;
  if (gsap26) {
    MotionPathPlugin2 = gsap26.plugins.motionPath;
    MotionPathHelper.PathEditor = PathEditor;
    _context8 = gsap26.core.context || function() {
    };
  }
  if (!MotionPathPlugin2) {
    required === true && console.warn(message);
  } else {
    _initCopyToClipboard();
    _arrayToRawPath = MotionPathPlugin2.arrayToRawPath;
    _rawPathToString = MotionPathPlugin2.rawPathToString;
  }
};
var MotionPathHelper = /* @__PURE__ */ (function() {
  function MotionPathHelper2(targetOrTween, vars) {
    var _this = this;
    if (vars === void 0) {
      vars = {};
    }
    if (!MotionPathPlugin2) {
      _initCore39(vars.gsap, 1);
    }
    var copyButton = _createElement7("div"), self = this, offset = {
      x: 0,
      y: 0
    }, target, path, isSVG, startX, startY, position, svg, animation, svgNamespace, temp, matrix, refreshPath, animationToScrub, createdSVG;
    if (targetOrTween instanceof gsap26.core.Tween) {
      animation = targetOrTween;
      target = animation.targets()[0];
    } else {
      target = gsap26.utils.toArray(targetOrTween)[0];
      animation = _findMotionPathTween(target);
    }
    path = _parsePath(vars.path, target, vars);
    this.offset = offset;
    position = _getPositionOnPage(target);
    startX = parseFloat(gsap26.getProperty(target, "x", "px"));
    startY = parseFloat(gsap26.getProperty(target, "y", "px"));
    isSVG = target.getCTM && target.tagName.toLowerCase() !== "svg";
    if (animation && !path) {
      path = _parsePath(animation.vars.motionPath.path || animation.vars.motionPath, target, animation.vars.motionPath);
    }
    copyButton.setAttribute("class", "copy-motion-path");
    copyButton.style.cssText = "border-radius:8px; background-color:rgba(85, 85, 85, 0.7); color:#fff; cursor:pointer; padding:6px 12px; font-family:Signika Negative, Arial, sans-serif; position:fixed; left:50%; transform:translate(-50%, 0); font-size:19px; bottom:10px";
    copyButton.innerText = "COPY MOTION PATH";
    copyButton._gsHelper = self;
    (gsap26.utils.toArray(vars.container)[0] || _body9).appendChild(copyButton);
    _addCopyToClipboard(copyButton, function() {
      return self.getString();
    }, function() {
      return gsap26.fromTo(copyButton, {
        backgroundColor: "white"
      }, {
        duration: 0.5,
        backgroundColor: "rgba(85, 85, 85, 0.6)"
      });
    });
    svg = path && path.ownerSVGElement;
    if (!svg) {
      svgNamespace = isSVG && target.ownerSVGElement && target.ownerSVGElement.getAttribute("xmlns") || "http://www.w3.org/2000/svg";
      if (isSVG) {
        svg = target.ownerSVGElement;
        temp = target.getBBox();
        matrix = _getConsolidatedMatrix3(target);
        startX = matrix.e;
        startY = matrix.f;
        offset.x = temp.x;
        offset.y = temp.y;
      } else {
        svg = _createElement7("svg", svgNamespace);
        createdSVG = true;
        _body9.appendChild(svg);
        svg.setAttribute("viewBox", "0 0 100 100");
        svg.setAttribute("class", "motion-path-helper");
        svg.style.cssText = "overflow:visible; background-color: transparent; position:absolute; z-index:5000; width:100px; height:100px; top:" + (position.top - startY) + "px; left:" + (position.left - startX) + "px;";
      }
      temp = _isString13(path) && !_selectorExp3.test(path) ? path : _getInitialPath(startX, startY);
      path = _createElement7("path", svgNamespace);
      path.setAttribute("d", temp);
      path.setAttribute("vector-effect", "non-scaling-stroke");
      path.style.cssText = "fill:transparent; stroke-width:" + (vars.pathWidth || 3) + "; stroke:" + (vars.pathColor || "#555") + "; opacity:" + (vars.pathOpacity || 0.6);
      svg.appendChild(path);
    } else {
      vars.pathColor && gsap26.set(path, {
        stroke: vars.pathColor
      });
      vars.pathWidth && gsap26.set(path, {
        strokeWidth: vars.pathWidth
      });
      vars.pathOpacity && gsap26.set(path, {
        opacity: vars.pathOpacity
      });
    }
    if (offset.x || offset.y) {
      gsap26.set(path, {
        x: offset.x,
        y: offset.y
      });
    }
    if (!("selected" in vars)) {
      vars.selected = true;
    }
    if (!("anchorSnap" in vars)) {
      vars.anchorSnap = function(p2) {
        if (p2.x * p2.x + p2.y * p2.y < 16) {
          p2.x = p2.y = 0;
        }
      };
    }
    animationToScrub = animation && animation.parent && animation.parent.data === "nested" ? animation.parent.parent : animation;
    vars.onPress = function() {
      animationToScrub.pause(0);
    };
    refreshPath = function refreshPath2() {
      if (_this.editor._anchors.length < 2) {
        console.warn("A motion path must have at least two anchors.");
      } else {
        animation.invalidate();
        animationToScrub.restart();
      }
    };
    vars.onRelease = vars.onDeleteAnchor = refreshPath;
    this.editor = PathEditor.create(path, vars);
    if (vars.center) {
      gsap26.set(target, {
        transformOrigin: "50% 50%",
        xPercent: -50,
        yPercent: -50
      });
    }
    if (animation) {
      if (animation.vars.motionPath.path) {
        animation.vars.motionPath.path = path;
      } else {
        animation.vars.motionPath = {
          path
        };
      }
      if (animationToScrub.parent !== gsap26.globalTimeline) {
        gsap26.globalTimeline.add(animationToScrub, _getGlobalTime(animationToScrub) - animationToScrub.delay());
      }
      animationToScrub.repeat(-1).repeatDelay(1);
    } else {
      animation = animationToScrub = gsap26.to(target, {
        motionPath: {
          path,
          start: vars.start || 0,
          end: "end" in vars ? vars.end : 1,
          autoRotate: "autoRotate" in vars ? vars.autoRotate : false,
          align: path,
          alignOrigin: vars.alignOrigin
        },
        duration: vars.duration || 5,
        ease: vars.ease || "power1.inOut",
        repeat: -1,
        repeatDelay: 1,
        paused: !vars.path
      });
    }
    this.animation = animation;
    _context8(this);
    this.kill = this.revert = function() {
      _this.editor.kill();
      copyButton.parentNode && copyButton.parentNode.removeChild(copyButton);
      createdSVG && svg.parentNode && svg.parentNode.removeChild(svg);
      animationToScrub && animationToScrub.revert();
    };
  }
  var _proto = MotionPathHelper2.prototype;
  _proto.getString = function getString() {
    return this.editor.getString(true, -this.offset.x, -this.offset.y);
  };
  return MotionPathHelper2;
})();
MotionPathHelper.register = _initCore39;
MotionPathHelper.create = function(target, vars) {
  return new MotionPathHelper(target, vars);
};
MotionPathHelper.editPath = function(path, vars) {
  return PathEditor.create(path, vars);
};
MotionPathHelper.version = "3.15.0";

// examples/figma-launch/node_modules/gsap/GSDevTools.js
var gsap27;
var _coreInitted23;
var _doc13;
var _docEl6;
var _win13;
var _recordedRoot;
var Animation2;
var _rootTween;
var _rootInstance;
var _keyboardInstance;
var _globalTimeline2;
var _independentRoot;
var _delayedCall;
var _context9;
var _recording = true;
var _startupPhase = true;
var _globalStartTime = 0;
var _windowExists21 = function _windowExists22() {
  return typeof window !== "undefined";
};
var _getGSAP43 = function _getGSAP44() {
  return gsap27 || _windowExists21() && (gsap27 = window.gsap) && gsap27.registerPlugin && gsap27;
};
var _isString15 = function _isString16(value) {
  return typeof value === "string";
};
var _isFunction15 = function _isFunction16(value) {
  return typeof value === "function";
};
var _isObject9 = function _isObject10(value) {
  return typeof value === "object";
};
var _isUndefined7 = function _isUndefined8(value) {
  return typeof value === "undefined";
};
var _svgNS = "http://www.w3.org/2000/svg";
var _domNS = "http://www.w3.org/1999/xhtml";
var _idSeed = 0;
var _lookup3 = {};
var _supportsStorage = (function() {
  try {
    sessionStorage.setItem("gsTest", "1");
    sessionStorage.removeItem("gsTest");
    return true;
  } catch (e) {
    return false;
  }
})();
var _parseAnimation = function _parseAnimation2(animationOrId) {
  return animationOrId instanceof Animation2 ? animationOrId : animationOrId ? gsap27.getById(animationOrId) : null;
};
var _createElement9 = function _createElement10(type, container, cssText) {
  var element = _doc13.createElementNS ? _doc13.createElementNS(type === "svg" ? _svgNS : _domNS, type) : _doc13.createElement(type);
  if (container) {
    if (_isString15(container)) {
      container = _doc13.querySelector(container);
    }
    container.appendChild(element);
  }
  if (type === "svg") {
    element.setAttribute("xmlns", _svgNS);
    element.setAttribute("xmlns:xlink", _domNS);
  }
  cssText && (element.style.cssText = cssText);
  return element;
};
var _clearSelection = function _clearSelection2() {
  if (_doc13.selection) {
    _doc13.selection.empty();
  } else if (_win13.getSelection) {
    _win13.getSelection().removeAllRanges();
  }
};
var _getChildrenOf = function _getChildrenOf2(timeline2, includeTimelines) {
  var a = [], cnt = 0, Tween2 = gsap27.core.Tween, tween = timeline2._first;
  while (tween) {
    if (tween instanceof Tween2) {
      if (tween.vars.id) {
        a[cnt++] = tween;
      }
    } else {
      if (includeTimelines && tween.vars.id) {
        a[cnt++] = tween;
      }
      a = a.concat(_getChildrenOf2(tween, includeTimelines));
      cnt = a.length;
    }
    tween = tween._next;
  }
  return a;
};
var _getClippedDuration = function _getClippedDuration2(animation, excludeRootRepeats) {
  var max = 0, repeat = Math.max(0, animation._repeat), t = animation._first;
  if (!t) {
    max = animation.duration();
  }
  while (t) {
    max = Math.max(max, t.totalDuration() > 999 ? t.endTime(false) : t._start + t._tDur / t._ts);
    t = t._next;
  }
  return !excludeRootRepeats && repeat ? max * (repeat + 1) + animation._rDelay * repeat : max;
};
var _globalizeTime = function _globalizeTime2(animation, rawTime) {
  var a = animation, time = arguments.length > 1 ? +rawTime : a.rawTime();
  while (a) {
    time = a._start + time / (a._ts || 1);
    a = a.parent;
  }
  return time;
};
var _timeToProgress = function _timeToProgress2(time, animation, defaultValue, relativeProgress) {
  var add, i2, a;
  if (_isString15(time)) {
    if (time.charAt(1) === "=") {
      add = parseInt(time.charAt(0) + "1", 10) * parseFloat(time.substr(2));
      if (add < 0 && relativeProgress === 0) {
        relativeProgress = 100;
      }
      time = relativeProgress / 100 * animation.duration() + add;
    } else if (isNaN(time) && animation.labels && animation.labels[time] !== -1) {
      time = animation.labels[time];
    } else if (animation === _recordedRoot) {
      i2 = time.indexOf("=");
      if (i2 > 0) {
        add = parseInt(time.charAt(i2 - 1) + "1", 10) * parseFloat(time.substr(i2 + 1));
        time = time.substr(0, i2 - 1);
      } else {
        add = 0;
      }
      a = gsap27.getById(time);
      if (a) {
        time = _globalizeTime(a, defaultValue / 100 * a.duration()) + add;
      }
    }
  }
  time = isNaN(time) ? defaultValue : parseFloat(time);
  return Math.min(100, Math.max(0, time / animation.duration() * 100));
};
var _addedCSS;
var _createRootElement = function _createRootElement2(element, minimal, css) {
  if (!_addedCSS) {
    _createElement9("style", _docEl6).innerHTML = ".gs-dev-tools{height:51px;bottom:0;left:0;right:0;display:block;position:fixed;overflow:visible;padding:0;font-size:15px;font-family:-apple-system,BlinkMacSystemFont,avenir next,sans-serif;color:#bbbaa6}.gs-dev-tools *{box-sizing:content-box;visibility:visible}.gs-dev-tools .gs-top{position:relative;z-index:499}.gs-dev-tools .gs-bottom{display:flex;align-items:center;justify-content:space-between;gap:1rem;background-color:#0e100f;height:42px;position:relative}.gs-dev-tools .timeline{position:relative;height:8px;margin-left:15px;margin-right:15px;overflow:visible}.gs-dev-tools .progress-bar,.gs-dev-tools .timeline-track{height:8px;width:100%;position:absolute;top:0;left:0;}.gs-dev-tools .timeline-track{background-color:#222}.gs-dev-tools .progress-bar{background:linear-gradient(114.41deg,#0ae448 20.74%,#abff84 65.5%);height:8px;top:0;width:0;pointer-events:none}.gs-dev-tools .seek-bar{width:100%;position:absolute;height:24px;top:-12px;left:0;background-color:transparent}.gs-dev-tools .in-point,.gs-dev-tools .out-point{width:15px;height:26px;position:absolute;top:-18px}.gs-dev-tools .in-point-shape{fill:#0ae448;transform:translateX(1px)}.gs-dev-tools .out-point-shape{fill:#ff8709}.gs-dev-tools .in-point{transform:translateX(-100%)}.gs-dev-tools .out-point{left:100%}.gs-dev-tools .playhead{position:absolute;top:-5px;transform:translate(-50%,0);left:0;border-radius:50%;width:16px;height:16px;background:linear-gradient(114.41deg,#0ae448 20.74%,#abff84 65.5%)}.gs-dev-tools .gs-btn-white{fill:#fffce1}.gs-dev-tools .pause{opacity:0}.gs-dev-tools .select-animation{vertical-align:middle;position:relative;padding:6px 10px}.gs-dev-tools .select-animation-container{flex-grow:4;width:40%}.gs-dev-tools .select-arrow{display:inline-block;width:12px;height:7px;margin:0 7px;transform:translate(0,-2px)}.gs-dev-tools .select-arrow-shape{stroke:currentcolor;stroke-width:2px;fill:none}.gs-dev-tools .rewind{height:14px}.gs-dev-tools .ease-border,.gs-dev-tools .rewind-path{fill:currentColor}.gs-dev-tools .play-pause{width:18px;height:18px}.gs-dev-tools .ease{width:20px;height:20px;min-width:30px;display:none}.gs-dev-tools .ease-path{fill:none;stroke:#abff84;stroke-width:2px}.gs-dev-tools .time-scale{text-align:center;min-width:30px}.gs-dev-tools .loop{width:15px}.gs-dev-tools label span{text-decoration:none}.gs-dev-tools button:focus,.gs-dev-tools select:focus{outline:0}.gs-dev-tools label{position:relative;cursor:pointer}.gs-dev-tools label.locked{text-decoration:none;cursor:auto}.gs-dev-tools label input,.gs-dev-tools label select{position:absolute;left:0;top:0;z-index:1;font:inherit;font-size:inherit;line-height:inherit;height:100%;width:100%;color:#000!important;opacity:0;background:0 0;border:none;padding:0;margin:0;-webkit-appearance:none;-moz-appearance:none;appearance:none;cursor:pointer}.gs-dev-tools label input+.display{position:relative;z-index:2}.gs-dev-tools .gs-bottom-right{vertical-align:middle;display:flex;align-items:center;flex-grow:4;width:40%;justify-content:flex-end}.gs-dev-tools .time-container{margin:0 5px}.gs-dev-tools .logo{width:32px;height:32px;position:relative;top:2px;margin:0 12px}.gs-dev-tools .gs-hit-area{background-color:transparent;width:100%;height:100%;top:0;position:absolute}.gs-dev-tools.minimal{border-top: 1px #222222 solid;height:auto;display:flex;align-items:stretch}.gs-dev-tools.minimal .gs-top{order:2;flex-grow:4;background-color:#0e100f}.gs-dev-tools.minimal .gs-bottom{background-color:#0e100f;border-top:none}.gs-dev-tools.minimal .timeline{top:50%;transform:translate(0,-50%)}.gs-dev-tools.minimal .gs-bottom-right,.gs-dev-tools.minimal .in-point,.gs-dev-tools.minimal .out-point,.gs-dev-tools.minimal .rewind,.gs-dev-tools.minimal .select-animation-container{display:none}.gs-dev-tools.minimal .play-pause{width:20px;height:20px;padding:4px 6px;margin-left:14px}.gs-dev-tools.minimal .time-scale{min-width:26px}.gs-dev-tools.minimal .loop{width:18px;min-width:18px;display:none}@media only screen and (max-width:600px){.gs-dev-tools{height:auto;display:flex;align-items:stretch}.gs-dev-tools .gs-top{order:2;flex-grow:4;background-color:#0e100f;height:42px}.gs-dev-tools .gs-bottom{background-color:#0e100f;border-top:none}.gs-dev-tools .timeline{top:50%;transform:translate(0,-50%)}.gs-dev-tools .gs-bottom-right,.gs-dev-tools .in-point,.gs-dev-tools .out-point,.gs-dev-tools .rewind,.gs-dev-tools .select-animation-container{display:none}.gs-dev-tools .play-pause{width:18px;height:18px;padding:4px 6px;margin-left:14px}.gs-dev-tools .time-scale{min-width:26px}.gs-dev-tools .loop{width:18px;min-width:18px;display:none}.gs-dev-tools .progress-bar,.gs-dev-tools .timeline-track{right:0}}";
    _addedCSS = true;
  }
  if (_isString15(element)) {
    element = _doc13.querySelector(element);
  }
  var root = _createElement9("div", element || _docEl6.getElementsByTagName("body")[0] || _docEl6);
  root.setAttribute("class", "gs-dev-tools" + (minimal ? " minimal" : ""));
  root.innerHTML = '<div class=gs-hit-area></div><div class=gs-top><div class=timeline><div class=timeline-track></div><div class=progress-bar></div><div class=seek-bar></div><svg class=in-point viewBox="0 0 15 26" xmlns=http://www.w3.org/2000/svg><path class=in-point-shape d="M0.5,2.283c0,-0.985 0.798,-1.783 1.783,-1.783c2.679,0 7.717,0 10.41,0c0.48,-0 0.939,0.19 1.278,0.529c0.339,0.339 0.529,0.798 0.529,1.277c-0,4.821 -0,17.897 0,21.968c0,0.253 -0.135,0.488 -0.354,0.615c-0.22,0.128 -0.49,0.128 -0.711,0.003c-2.653,-1.517 -9.526,-5.444 -12.016,-6.867c-0.568,-0.325 -0.919,-0.929 -0.919,-1.583c-0,-2.835 -0,-10.627 -0,-14.159Z" style="fill:#00ff52;fill-rule:nonzero;"/></svg><svg class=out-point viewBox="0 0 15 26" xmlns=http://www.w3.org/2000/svg><path class=out-point-shape d="M0.5,2.251c0,-0.465 0.184,-0.91 0.513,-1.238c0.328,-0.329 0.773,-0.513 1.238,-0.513c2.669,0 7.733,0 10.439,0c0.48,-0 0.94,0.191 1.28,0.53c0.339,0.34 0.53,0.8 0.53,1.28l0,14.17c-0,0.631 -0.338,1.213 -0.886,1.526c-2.44,1.395 -9.262,5.293 -11.977,6.845c-0.236,0.134 -0.524,0.133 -0.759,-0.003c-0.234,-0.136 -0.378,-0.386 -0.378,-0.657c0,-4.178 0,-17.198 0,-21.94Z" style="fill-rule:nonzero;"/></svg><div class=playhead></div></div></div><div class=gs-bottom><div class=select-animation-container><label class=select-animation><select class=animation-list><option>Global Timeline<option>myTimeline</select><nobr><span class="display animation-label">Global Timeline</span><svg class=select-arrow viewBox="0 0 12.05 6.73" xmlns=http://www.w3.org/2000/svg><polyline class=select-arrow-shape points="0.35 0.35 6.03 6.03 11.7 0.35"/></svg></nobr></label></div><svg class=rewind viewBox="0 0 12 15.38" xmlns=http://www.w3.org/2000/svg><path d=M0,.38H2v15H0Zm2,7,10,7.36V0Z class="gs-btn-white rewind-path"/></svg><svg class=play-pause viewBox="0 0 20.97 25.67" xmlns=http://www.w3.org/2000/svg><g class=play><path d="M8,4.88 C8,10.18 8,15.48 8,20.79 5.33,22.41 2.66,24.04 0,25.67 0,17.11 0,8.55 0,0 2.66,1.62 5.33,3.25 8,4.88" class="gs-btn-white play-1" style=stroke:#fffce1;stroke-width:.6px /><path d="M14.485,8.855 C16.64,10.18 18.8,11.5 20.97,12.83 16.64,15.48 12.32,18.13 8,20.79 8,15.48 8,10.18 8,4.88 10.16,6.2 12.32,7.53 14.48,8.85" class="gs-btn-white play-2" style=stroke:#fffce1;stroke-width:.6px /></g></svg> <svg class=loop viewBox="0 0 29 25.38" xmlns=http://www.w3.org/2000/svg fill="currentcolor"><path d=M27.44,5.44,20.19,0V3.06H9.06A9.31,9.31,0,0,0,0,12.41,9.74,9.74,0,0,0,.69,16l3.06-2.23a6,6,0,0,1-.12-1.22,5.49,5.49,0,0,1,5.43-5.5H20.19v3.81Z class=loop-path /><path d=M25.25,11.54a5.18,5.18,0,0,1,.12,1.12,5.41,5.41,0,0,1-5.43,5.41H9.19V14.5L1.94,19.94l7.25,5.44V22.06H19.94A9.2,9.2,0,0,0,29,12.84a9.42,9.42,0,0,0-.68-3.53Z class=loop-path /></svg> <svg class=ease viewBox="0 0 25.67 25.67" xmlns=http://www.w3.org/2000/svg><path d=M.48,25.12c1.74-3.57,4.28-12.6,8.8-10.7s4.75,1.43,6.5-1.11S19.89,1.19,25.2.55 class=ease-path /><path d=M24.67,1V24.67H1V1H24.67m1-1H0V25.67H25.67V0Z class=ease-border /></svg><label class=time-scale><select><option value=10>10x<option value=5>5x<option value=2>2x<option value=1 selected>1x<option value=0.5>0.5x<option value=0.25>0.25x<option value=0.1>0.1x</select><span class="display time-scale-label">1x</span></label><div class=gs-bottom-right><div class=time-container><span class=time>0.00</span> / <span class=duration>0.00</span></div><a href="https://gsap.com/docs/v3/Plugins/GSDevTools?source=GSDevTools" target=_blank title=Docs><svg class="logo" viewBox="0 0 1080 1080" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M793 518.654C793 518.654 793 518.829 793 518.934L770.197 618.386C768.967 624.012 763.38 628.135 756.915 628.135H729.404C727.366 628.135 725.539 629.498 724.977 631.455C699.573 718.082 665.175 777.628 619.745 813.376C581.095 843.813 533.451 858 469.819 858C412.618 858 374.074 839.514 341.362 803.032C298.145 754.809 280.261 675.869 291.083 580.75C310.618 402.184 402.745 222.01 580.217 222.01C634.185 221.521 676.559 238.26 706.039 271.667C737.204 306.995 753.05 360.216 753.121 429.86C753.015 436.185 747.78 441.287 741.491 441.287H611.488C606.885 441.287 602.774 437.129 602.844 432.551C601.79 384.397 587.56 360.915 559.311 360.915C509.488 360.915 480.079 428.917 464.479 466.622C442.694 519.283 431.627 576.452 433.805 633.412C434.824 659.935 439.075 697.22 464.127 712.666C486.333 726.364 518.026 717.279 537.21 702.113C556.394 686.947 571.819 660.703 578.319 636.766C579.233 633.447 579.303 630.861 578.425 629.708C577.511 628.52 574.981 628.24 573.049 628.24H539.67C536.086 628.24 532.537 626.598 530.394 623.942C528.602 621.705 527.899 618.945 528.532 616.219L551.37 516.592C552.494 511.49 557.097 507.647 562.754 506.948V506.703H781.827C782.354 506.703 782.881 506.703 783.373 506.808C789.065 507.542 793.07 512.853 792.965 518.654H793Z" fill="#0AE448"/></svg></a></div></div>';
  if (element) {
    root.style.position = "absolute";
    root.style.top = minimal ? "calc(100% - 42px)" : "calc(100% - 51px)";
  }
  if (css) {
    if (_isString15(css)) {
      root.style.cssText = css;
    } else if (_isObject9(css)) {
      css.data = "root";
      gsap27.set(root, css).kill();
    }
    if (root.style.top) {
      root.style.bottom = "auto";
    }
    if (root.style.width) {
      gsap27.set(root, {
        xPercent: -50,
        left: "50%",
        right: "auto",
        data: "root"
      }).kill();
    }
  }
  if (!minimal && root.offsetWidth < 600) {
    root.setAttribute("class", "gs-dev-tools minimal");
    if (element) {
      root.style.top = "calc(100% - 42px)";
    }
  }
  return root;
};
var _clickedOnce = true;
var _addListener9 = function _addListener10(e, type, callback, capture) {
  var handler, altType;
  if (type === "mousedown" || type === "mouseup") {
    e.style.cursor = "pointer";
  }
  if (type === "mousedown") {
    altType = !_isUndefined7(e.onpointerdown) ? "pointerdown" : !_isUndefined7(e.ontouchstart) ? "touchstart" : null;
    if (altType) {
      handler = function handler2(event) {
        if (event.target.nodeName.toLowerCase() !== "select" && event.type === altType) {
          event.stopPropagation();
          if (_clickedOnce) {
            event.preventDefault();
            callback.call(e, event);
          }
        } else if (event.type !== altType) {
          callback.call(e, event);
        }
        _clickedOnce = true;
      };
      e.addEventListener(altType, handler, capture);
      if (altType !== "pointerdown") {
        e.addEventListener(type, handler, capture);
      }
      return;
    }
  }
  e.addEventListener(type, callback, capture);
};
var _removeListener9 = function _removeListener10(e, type, callback) {
  e.removeEventListener(type, callback);
  type = type !== "mousedown" ? null : !_isUndefined7(e.onpointerdown) ? "pointerdown" : !_isUndefined7(e.ontouchstart) ? "touchstart" : null;
  if (type) {
    e.removeEventListener(type, callback);
  }
};
var _selectValue = function _selectValue2(element, value, label, insertIfAbsent) {
  var options = element.options, i2 = options.length, option;
  value += "";
  while (--i2 > -1) {
    if (options[i2].innerHTML === value || options[i2].value === value) {
      element.selectedIndex = i2;
      label.innerHTML = options[i2].innerHTML;
      return options[i2];
    }
  }
  if (insertIfAbsent) {
    option = _createElement9("option", element);
    option.setAttribute("value", value);
    option.innerHTML = label.innerHTML = _isString15(insertIfAbsent) ? insertIfAbsent : value;
    element.selectedIndex = options.length - 1;
  }
};
var _shiftSelectedValue = function _shiftSelectedValue2(element, amount, label) {
  var options = element.options, i2 = Math.min(options.length - 1, Math.max(0, element.selectedIndex + amount));
  element.selectedIndex = i2;
  if (label) {
    label.innerHTML = options[i2].innerHTML;
  }
  return options[i2].value;
};
var _merge3 = function _merge4() {
  var t = _globalTimeline2._first, duration, next, target;
  if (_rootInstance) {
    duration = _recordedRoot._dur;
    while (t) {
      next = t._next;
      target = t._targets && t._targets[0];
      if (!(_isFunction15(target) && target === t.vars.onComplete && !t._dur) && !(target && target._gsIgnore)) {
        _recordedRoot.add(t, t._start - t._delay);
      }
      t = next;
    }
    return duration !== _recordedRoot.duration();
  }
};
var _buildPlayPauseMorph = function _buildPlayPauseMorph2(svg) {
  var tl = gsap27.timeline({
    data: "root",
    parent: _independentRoot,
    onComplete: function onComplete() {
      return tl.kill();
    }
  }, _independentRoot._time);
  tl.to(svg.querySelector(".play-1"), {
    duration: 0.4,
    attr: {
      d: "M5.75,3.13 C5.75,9.79 5.75,16.46 5.75,23.13 4.08,23.13 2.41,23.13 0.75,23.13 0.75,16.46 0.75,9.79 0.75,3.12 2.41,3.12 4.08,3.12 5.75,3.12"
    },
    ease: "power2.inOut",
    rotation: 360,
    transformOrigin: "50% 50%"
  }).to(svg.querySelector(".play-2"), {
    duration: 0.4,
    attr: {
      d: "M16.38,3.13 C16.38,9.79 16.38,16.46 16.38,23.13 14.71,23.13 13.04,23.13 11.38,23.13 11.38,16.46 11.38,9.79 11.38,3.12 13.04,3.12 14.71,3.12 16.38,3.12"
    },
    ease: "power2.inOut",
    rotation: 360,
    transformOrigin: "50% 50%"
  }, 0.05);
  return tl;
};
var _buildLoopAnimation = function _buildLoopAnimation2(svg) {
  var tl = gsap27.timeline({
    data: "root",
    id: "loop",
    parent: _independentRoot,
    paused: true,
    onComplete: function onComplete() {
      return tl.kill();
    }
  }, _independentRoot._time);
  tl.to(svg, {
    duration: 0.5,
    rotation: 360,
    ease: "power3.inOut",
    transformOrigin: "50% 50%"
  }).to(svg.querySelectorAll(".loop-path"), {
    duration: 0.5,
    fill: "#91e600",
    ease: "none"
  }, 0);
  return tl;
};
var _getAnimationById = function _getAnimationById2(id) {
  return gsap27.getById(id) || _independentRoot.getById(id) || id === _recordedRoot.vars.id && _recordedRoot;
};
var _initCore41 = function _initCore42(core) {
  gsap27 = core || _getGSAP43();
  if (!_coreInitted23) {
    if (gsap27 && _windowExists21()) {
      _doc13 = document;
      _docEl6 = _doc13.documentElement;
      _win13 = window;
      _context9 = gsap27.core.context || function() {
      };
      gsap27.registerPlugin(Draggable);
      _globalTimeline2 = gsap27.globalTimeline;
      _globalTimeline2._sort = true;
      _globalTimeline2.autoRemoveChildren = false;
      Animation2 = gsap27.core.Animation;
      _independentRoot = gsap27.timeline({
        data: "indy",
        autoRemoveChildren: true,
        smoothChildTiming: true
      });
      _independentRoot.kill();
      _independentRoot._dp = 0;
      _independentRoot.to({}, {
        duration: 1e12
      });
      _recordedRoot = gsap27.timeline({
        data: "root",
        id: "Global Timeline",
        autoRemoveChildren: false,
        smoothChildTiming: true,
        parent: _independentRoot
      }, 0);
      _rootTween = gsap27.to(_recordedRoot, {
        duration: 1,
        time: 1,
        ease: "none",
        data: "root",
        id: "_rootTween",
        paused: true,
        immediateRender: false,
        parent: _independentRoot
      }, 0);
      _globalTimeline2.killTweensOf = function(targets, props, onlyActive) {
        _recordedRoot.killTweensOf(targets, props, onlyActive);
        _recordedRoot.killTweensOf.call(_globalTimeline2, targets, props, onlyActive);
      };
      _independentRoot._start = gsap27.ticker.time;
      gsap27.ticker.add(function(time) {
        return _independentRoot.render(time - _independentRoot._start);
      });
      _globalTimeline2._start += _globalTimeline2._time;
      _recordedRoot._start = _globalTimeline2._time = _globalTimeline2._tTime = 0;
      _delayedCall = function _delayedCall2(delay, callback, params, scope) {
        return gsap27.to(callback, {
          delay,
          duration: 0,
          onComplete: callback,
          onReverseComplete: callback,
          onCompleteParams: params,
          onReverseCompleteParams: params,
          callbackScope: scope,
          parent: _independentRoot
        }, _independentRoot._time);
      };
      _delayedCall(0.01, function() {
        return _rootInstance ? _rootInstance.update() : _merge3();
      });
      _delayedCall(2, function() {
        var t, next, offset;
        if (!_rootInstance) {
          _merge3();
          t = _recordedRoot._first;
          offset = _recordedRoot._start;
          while (t) {
            next = t._next;
            if (t._tDur !== t._tTime || !t._dur && t.progress() !== 1) {
              _globalTimeline2.add(t, t._start - t._delay + offset);
            } else {
              t.kill();
            }
            t = next;
          }
        }
        if (GSDevTools.globalRecordingTime > 2) {
          _delayedCall(GSDevTools.globalRecordingTime - 2, function() {
            _rootInstance && _rootInstance.update();
            _recording = false;
            _globalTimeline2.autoRemoveChildren = true;
          });
        } else {
          _recording = false;
          _globalTimeline2.autoRemoveChildren = true;
        }
        _startupPhase = false;
      });
      _coreInitted23 = 1;
    }
  }
};
var _checkIndependence = function _checkIndependence2(animation, vars) {
  if (!vars.globalSync && animation.parent !== _globalTimeline2) {
    _globalTimeline2.add(animation, _globalTimeline2.time());
  }
};
var GSDevTools = function GSDevTools2(vars) {
  if (!_coreInitted23) {
    _initCore41();
    gsap27 || console.warn("Please gsap.registerPlugin(GSDevTools)");
  }
  this.vars = vars = vars || {};
  if (vars.animation) {
    (GSDevTools2.getByAnimation(vars.animation) || {
      kill: function kill5() {
        return 0;
      }
    }).kill();
  }
  vars.id = vars.id || (_isString15(vars.animation) ? vars.animation : _idSeed++);
  _lookup3[vars.id + ""] = this;
  "globalSync" in vars || (vars.globalSync = !vars.animation);
  var _self = this, root = _createRootElement(vars.container, vars.minimal, vars.css), find = function find2(s) {
    return root.querySelector(s);
  }, record = function record2(key, value) {
    if (vars.persist !== false && _supportsStorage) {
      sessionStorage.setItem("gs-dev-" + key + vars.id, value);
    }
    return value;
  }, recall = function recall2(key) {
    var value;
    if (vars.persist !== false && _supportsStorage) {
      value = sessionStorage.getItem("gs-dev-" + key + vars.id);
      return key === "animation" ? value : key === "loop" ? value === "true" : parseFloat(value);
    }
  }, playhead = find(".playhead"), timelineTrack = find(".timeline-track"), progressBar = find(".progress-bar"), timeLabel = find(".time"), durationLabel = find(".duration"), pixelToTimeRatio, timeAtDragStart, dragged, skipDragUpdates, progress = 0, inPoint = find(".in-point"), outPoint = find(".out-point"), inProgress = 0, outProgress = 100, pausedWhenDragStarted, list = find(".animation-list"), animationLabel = find(".animation-label"), selectedAnimation, linkedAnimation, declaredAnimation, startTime, endTime, _fullyInitialized, keyboardHandler, playPauseButton = find(".play-pause"), playPauseMorph = _buildPlayPauseMorph(playPauseButton), paused = false, loopButton = find(".loop"), loopAnimation = _buildLoopAnimation(loopButton), loopEnabled, timeScale = find(".time-scale select"), timeScaleLabel = find(".time-scale-label"), onPressTimeline = function onPressTimeline2(element, originRatio, limitToInOut) {
    return function(e) {
      var trackBounds = timelineTrack.getBoundingClientRect(), elementBounds = element.getBoundingClientRect(), left = elementBounds.width * originRatio, x = gsap27.getProperty(element, "x"), minX = trackBounds.left - elementBounds.left - left + x, maxX = trackBounds.right - elementBounds.right + (elementBounds.width - left) + x, unlimitedMinX = minX, limitBounds;
      if (limitToInOut) {
        if (element !== inPoint) {
          limitBounds = inPoint.getBoundingClientRect();
          if (limitBounds.left) {
            minX += limitBounds.left + limitBounds.width - trackBounds.left;
          }
        }
        if (element !== outPoint) {
          limitBounds = outPoint.getBoundingClientRect();
          if (limitBounds.left) {
            maxX -= trackBounds.left + trackBounds.width - limitBounds.left;
          }
        }
      }
      pausedWhenDragStarted = paused;
      this.applyBounds({
        minX,
        maxX
      });
      pixelToTimeRatio = linkedAnimation.duration() / trackBounds.width;
      timeAtDragStart = -unlimitedMinX * pixelToTimeRatio;
      if (!skipDragUpdates) {
        linkedAnimation.pause(timeAtDragStart + pixelToTimeRatio * this.x);
      } else {
        linkedAnimation.pause();
      }
      if (this.target === playhead) {
        if (this.activated) {
          this.allowEventDefault = false;
        }
        this.activated = true;
      }
      dragged = true;
    };
  }, progressDrag = Draggable.create(playhead, {
    type: "x",
    cursor: "ew-resize",
    allowNativeTouchScrolling: false,
    allowEventDefault: true,
    //otherwise, when dragged outside an iframe, the mouseup doesn't bubble up so it could seem "stuck" to the mouse.
    onPress: onPressTimeline(playhead, 0.5, true),
    onDrag: function onDrag() {
      var time = timeAtDragStart + pixelToTimeRatio * this.x;
      if (time < 0) {
        time = 0;
      } else if (time > linkedAnimation._dur) {
        time = linkedAnimation._dur;
      }
      if (!skipDragUpdates) {
        linkedAnimation.time(time);
      }
      progressBar.style.width = Math.min(outProgress - inProgress, Math.max(0, time / linkedAnimation._dur * 100 - inProgress)) + "%";
      timeLabel.innerHTML = time.toFixed(2);
    },
    onRelease: function onRelease() {
      paused || linkedAnimation.resume();
    }
  })[0], resetInOut = function resetInOut2() {
    inProgress = 0;
    outProgress = 100;
    inPoint.style.left = "0%";
    outPoint.style.left = "100%";
    record("in", inProgress);
    record("out", outProgress);
    updateProgress(true);
  }, inDrag = Draggable.create(inPoint, {
    type: "x",
    cursor: "ew-resize",
    zIndexBoost: false,
    allowNativeTouchScrolling: false,
    allowEventDefault: true,
    //otherwise, when dragged outside an iframe, the mouseup doesn't bubble up so it could seem "stuck" to the mouse.
    onPress: onPressTimeline(inPoint, 1, true),
    onDoubleClick: resetInOut,
    onDrag: function onDrag() {
      inProgress = (timeAtDragStart + pixelToTimeRatio * this.x) / linkedAnimation.duration() * 100;
      linkedAnimation.progress(inProgress / 100);
      updateProgress(true);
    },
    onRelease: function onRelease() {
      if (inProgress < 0) {
        inProgress = 0;
      }
      _clearSelection();
      inPoint.style.left = inProgress + "%";
      record("in", inProgress);
      gsap27.set(inPoint, {
        x: 0,
        data: "root",
        display: "block"
      });
      if (!paused) {
        linkedAnimation.resume();
      }
    }
  })[0], outDrag = Draggable.create(outPoint, {
    type: "x",
    cursor: "ew-resize",
    allowNativeTouchScrolling: false,
    allowEventDefault: true,
    //otherwise, when dragged outside an iframe, the mouseup doesn't bubble up so it could seem "stuck" to the mouse.
    zIndexBoost: false,
    onPress: onPressTimeline(outPoint, 0, true),
    onDoubleClick: resetInOut,
    onDrag: function onDrag() {
      outProgress = (timeAtDragStart + pixelToTimeRatio * this.x) / linkedAnimation.duration() * 100;
      linkedAnimation.progress(outProgress / 100);
      updateProgress(true);
    },
    onRelease: function onRelease() {
      if (outProgress > 100) {
        outProgress = 100;
      }
      _clearSelection();
      outPoint.style.left = outProgress + "%";
      record("out", outProgress);
      gsap27.set(outPoint, {
        x: 0,
        data: "root",
        display: "block"
      });
      if (!pausedWhenDragStarted) {
        play();
        linkedAnimation.resume();
      }
    }
  })[0], updateProgress = function updateProgress2(force) {
    if (progressDrag.isPressed && force !== true) {
      return;
    }
    var p2 = !loopEnabled && selectedAnimation._repeat === -1 ? selectedAnimation.totalTime() / selectedAnimation.duration() * 100 : linkedAnimation.progress() * 100 || 0, repeatDelayPhase = selectedAnimation._repeat && selectedAnimation._rDelay && selectedAnimation.totalTime() % (selectedAnimation.duration() + selectedAnimation._rDelay) > selectedAnimation.duration(), target;
    if (p2 > 100) {
      p2 = 100;
    }
    if (p2 >= outProgress) {
      if (loopEnabled && !linkedAnimation.paused() && !progressDrag.isDragging) {
        if (!repeatDelayPhase) {
          p2 = inProgress;
          target = linkedAnimation._targets && linkedAnimation._targets[0];
          if (target === selectedAnimation) {
            target.seek(startTime + (endTime - startTime) * inProgress / 100);
          }
          if (selectedAnimation._repeat > 0 && !inProgress && outProgress === 100) {
            if (selectedAnimation.totalProgress() === 1) {
              linkedAnimation.totalProgress(0, true).resume();
            }
          } else {
            linkedAnimation.progress(p2 / 100, true).resume();
          }
        }
      } else {
        if (p2 !== outProgress || selectedAnimation._repeat === -1) {
          p2 = outProgress;
          linkedAnimation.progress(p2 / 100);
        }
        if (!paused && (outProgress < 100 || selectedAnimation.totalProgress() === 1 || selectedAnimation._repeat === -1)) {
          pause();
        }
      }
    } else if (p2 < inProgress) {
      p2 = inProgress;
      linkedAnimation.progress(p2 / 100, true);
    }
    if (p2 !== progress || force === true) {
      progressBar.style.left = inProgress + "%";
      progressBar.style.width = Math.max(0, p2 - inProgress) + "%";
      playhead.style.left = p2 + "%";
      timeLabel.innerHTML = linkedAnimation._time.toFixed(2);
      durationLabel.innerHTML = linkedAnimation._dur.toFixed(2);
      if (dragged) {
        playhead.style.transform = "translate(-50%,0)";
        playhead._gsap.x = "0px";
        playhead._gsap.xPercent = -50;
        dragged = false;
      }
      progress = p2;
    }
    linkedAnimation.paused() !== paused && togglePlayPause();
  }, onPressSeekBar = function onPressSeekBar2(e) {
    if (progressDrag.isPressed) {
      return;
    }
    var bounds = e.target.getBoundingClientRect(), x = (e.changedTouches ? e.changedTouches[0] : e).clientX, p2 = (x - bounds.left) / bounds.width * 100;
    if (p2 < inProgress) {
      inProgress = p2 = Math.max(0, p2);
      inPoint.style.left = inProgress + "%";
      inDrag.startDrag(e);
      return;
    } else if (p2 > outProgress) {
      outProgress = p2 = Math.min(100, p2);
      outPoint.style.left = outProgress + "%";
      outDrag.startDrag(e);
      return;
    }
    linkedAnimation.progress(p2 / 100).pause();
    updateProgress(true);
    progressDrag.startDrag(e);
  }, play = function play2() {
    if (linkedAnimation.progress() >= outProgress / 100) {
      _checkIndependence(linkedAnimation, vars);
      var target = linkedAnimation._targets && linkedAnimation._targets[0];
      if (target === selectedAnimation) {
        target.seek(startTime + (endTime - startTime) * inProgress / 100);
      }
      if (linkedAnimation._repeat && !inProgress) {
        linkedAnimation.totalProgress(0, true);
      } else if (!linkedAnimation.reversed()) {
        linkedAnimation.progress(inProgress / 100, true);
      }
    }
    playPauseMorph.play();
    linkedAnimation.resume();
    if (paused) {
      _self.update();
    }
    paused = false;
  }, pause = function pause2() {
    playPauseMorph.reverse();
    if (linkedAnimation) {
      linkedAnimation.pause();
    }
    paused = true;
  }, togglePlayPause = function togglePlayPause2() {
    if (paused) {
      play();
    } else {
      pause();
    }
  }, onPressRewind = function onPressRewind2(e) {
    if (progressDrag.isPressed) {
      return;
    }
    _checkIndependence(linkedAnimation, vars);
    var target = linkedAnimation._targets && linkedAnimation._targets[0];
    if (target === selectedAnimation) {
      target.seek(startTime + (endTime - startTime) * inProgress / 100);
    }
    linkedAnimation.progress(inProgress / 100, true);
    if (!paused) {
      linkedAnimation.resume();
    }
  }, loop = function loop2(value) {
    loopEnabled = value;
    record("loop", loopEnabled);
    if (loopEnabled) {
      loopAnimation.play();
      if (linkedAnimation.progress() >= outProgress / 100) {
        var target = linkedAnimation._targets && linkedAnimation._targets[0];
        if (target === selectedAnimation) {
          target.seek(startTime + (endTime - startTime) * inProgress / 100);
        }
        if (selectedAnimation._repeat && !inProgress && outProgress === 100) {
          linkedAnimation.totalProgress(0, true);
        } else {
          linkedAnimation.progress(inProgress / 100, true);
        }
        play();
      }
    } else {
      loopAnimation.reverse();
    }
  }, toggleLoop = function toggleLoop2() {
    return loop(!loopEnabled);
  }, updateList = function updateList2() {
    var animations = _getChildrenOf(declaredAnimation && !vars.globalSync ? declaredAnimation : _recordedRoot, true), options = list.children, matches = 0, option, i2;
    if (declaredAnimation && !vars.globalSync) {
      animations.unshift(declaredAnimation);
    } else if (!vars.hideGlobalTimeline) {
      animations.unshift(_recordedRoot);
    }
    for (i2 = 0; i2 < animations.length; i2++) {
      option = options[i2] || _createElement9("option", list);
      option.animation = animations[i2];
      matches = i2 && animations[i2].vars.id === animations[i2 - 1].vars.id ? matches + 1 : 0;
      option.setAttribute("value", option.innerHTML = animations[i2].vars.id + (matches ? " [" + matches + "]" : animations[i2 + 1] && animations[i2 + 1].vars.id === animations[i2].vars.id ? " [0]" : ""));
    }
    for (; i2 < options.length; i2++) {
      list.removeChild(options[i2]);
    }
  }, animation = function animation2(anim) {
    var ts = parseFloat(timeScale.options[timeScale.selectedIndex].value) || 1, tl, maxDuration;
    if (!arguments.length) {
      return selectedAnimation;
    }
    if (_isString15(anim)) {
      anim = _getAnimationById(anim);
    }
    if (!(anim instanceof Animation2)) {
      console.warn("GSDevTools error: invalid animation.");
    }
    if (anim.scrollTrigger) {
      console.warn("GSDevTools can't work with ScrollTrigger-based animations; either the scrollbar -OR- the GSDevTools scrubber can control the animation.");
    }
    if (anim === selectedAnimation) {
      return;
    }
    if (selectedAnimation) {
      selectedAnimation._inProgress = inProgress;
      selectedAnimation._outProgress = outProgress;
    }
    selectedAnimation = anim;
    if (linkedAnimation) {
      ts = linkedAnimation.timeScale();
      if (linkedAnimation._targets && linkedAnimation._targets[0] === declaredAnimation) {
        declaredAnimation.resume();
        linkedAnimation.kill();
      }
    }
    inProgress = selectedAnimation._inProgress || 0;
    outProgress = selectedAnimation._outProgress || 100;
    inPoint.style.left = inProgress + "%";
    outPoint.style.left = outProgress + "%";
    if (_fullyInitialized) {
      record("animation", selectedAnimation.vars.id);
      record("in", inProgress);
      record("out", outProgress);
    }
    startTime = 0;
    maxDuration = vars.maxDuration || Math.min(1e3, _getClippedDuration(selectedAnimation));
    if (selectedAnimation === _recordedRoot || vars.globalSync) {
      _merge3();
      linkedAnimation = _rootTween;
      _rootInstance && _rootInstance !== _self && console.warn("Error: GSDevTools can only have one instance that's globally synchronized.");
      _rootInstance = _self;
      if (selectedAnimation !== _recordedRoot) {
        tl = selectedAnimation;
        endTime = tl.totalDuration();
        if (endTime > 99999999) {
          endTime = tl.duration();
        }
        while (tl.parent) {
          startTime = startTime / tl._ts + tl._start;
          endTime = endTime / tl._ts + tl._start;
          tl = tl.parent;
        }
      } else {
        endTime = _recordedRoot.duration();
      }
      if (endTime - startTime > maxDuration) {
        endTime = startTime + maxDuration;
      }
      _recordedRoot.pause(startTime);
      _rootTween.vars.time = endTime;
      _rootTween.invalidate();
      _rootTween.duration(endTime - startTime).timeScale(ts);
      if (paused) {
        _rootTween.progress(1, true).pause(0, true);
      } else {
        _delayedCall(0.01, function() {
          _rootTween.resume().progress(inProgress / 100);
          paused && play();
        });
      }
    } else {
      if (_rootInstance === _self) {
        _rootInstance = null;
      }
      startTime = Math.min(inProgress * selectedAnimation.duration(), selectedAnimation.time());
      if (selectedAnimation === declaredAnimation || !declaredAnimation) {
        linkedAnimation = selectedAnimation;
        if (!loopEnabled && linkedAnimation._repeat) {
          loop(true);
        }
      } else {
        tl = selectedAnimation;
        endTime = tl.totalDuration();
        if (endTime > 99999999) {
          endTime = tl.duration();
        }
        while (tl.parent && tl !== declaredAnimation) {
          startTime = startTime / (tl._ts || tl._rts) + tl._start;
          endTime = endTime / (tl._ts || tl._rts) + tl._start;
          tl = tl.parent;
        }
        if (endTime - startTime > maxDuration) {
          endTime = startTime + maxDuration;
        }
        declaredAnimation.pause(startTime);
        linkedAnimation = gsap27.to(declaredAnimation, {
          duration: endTime - startTime,
          time: endTime,
          ease: "none",
          data: "root",
          parent: _independentRoot
        }, _independentRoot._time);
      }
      linkedAnimation.timeScale(ts);
      _rootTween.pause();
      _recordedRoot.resume();
      linkedAnimation.seek(0);
    }
    durationLabel.innerHTML = linkedAnimation.duration().toFixed(2);
    _selectValue(list, selectedAnimation.vars.id, animationLabel);
  }, updateRootDuration = function updateRootDuration2() {
    var time, ratio, duration;
    if (selectedAnimation === _recordedRoot) {
      time = _recordedRoot._time;
      _recordedRoot.progress(1, true).time(time, true);
      time = (_rootTween._dp._time - _rootTween._start) * _rootTween._ts;
      duration = Math.min(1e3, _recordedRoot.duration());
      if (duration === 1e3) {
        duration = Math.min(1e3, _getClippedDuration(_recordedRoot));
      }
      ratio = _rootTween.duration() / duration;
      if (ratio !== 1 && duration) {
        inProgress *= ratio;
        if (outProgress < 100) {
          outProgress *= ratio;
        }
        _rootTween.seek(0);
        _rootTween.vars.time = duration;
        _rootTween.invalidate();
        _rootTween.duration(duration);
        _rootTween.time(time);
        durationLabel.innerHTML = duration.toFixed(2);
        inPoint.style.left = inProgress + "%";
        outPoint.style.left = outProgress + "%";
        updateProgress(true);
      }
    }
  }, onChangeAnimation = function onChangeAnimation2(e) {
    animation(list.options[list.selectedIndex].animation);
    if (e.target && e.target.blur) {
      e.target.blur();
    }
    paused && play();
  }, onChangeTimeScale = function onChangeTimeScale2(e) {
    var ts = parseFloat(timeScale.options[timeScale.selectedIndex].value) || 1, target;
    linkedAnimation.timeScale(ts);
    record("timeScale", ts);
    if (!paused) {
      if (linkedAnimation.progress() >= outProgress / 100) {
        target = linkedAnimation._targets && linkedAnimation._targets[0];
        if (target === selectedAnimation) {
          target.seek(startTime + (endTime - startTime) * inProgress / 100);
        }
        linkedAnimation.progress(inProgress / 100, true).pause();
      } else {
        linkedAnimation.pause();
      }
      _delayedCall(0.01, function() {
        return linkedAnimation.resume();
      });
    }
    timeScaleLabel.innerHTML = ts + "x";
    if (timeScale.blur) {
      timeScale.blur();
    }
  }, autoHideTween = gsap27.to([find(".gs-bottom"), find(".gs-top")], {
    duration: 0.3,
    autoAlpha: 0,
    y: 50,
    ease: "power2.in",
    data: "root",
    paused: true,
    parent: _independentRoot
  }, _independentRoot._time), hidden = false, onMouseOut = function onMouseOut2(e) {
    if (!Draggable.hitTest(e, root) && !progressDrag.isDragging && !inDrag.isDragging && !outDrag.isDragging) {
      autoHideDelayedCall.restart(true);
    }
  }, hide = function hide2() {
    if (!hidden) {
      autoHideTween.play();
      autoHideDelayedCall.pause();
      hidden = true;
    }
  }, show = function show2() {
    autoHideDelayedCall.pause();
    if (hidden) {
      autoHideTween.reverse();
      hidden = false;
    }
  }, toggleHide = function toggleHide2() {
    if (hidden) {
      show();
    } else {
      hide();
    }
  }, autoHideDelayedCall = _delayedCall(1.3, hide).pause(), initialize = function initialize2(preliminary) {
    if (_startupPhase && !_globalStartTime) {
      _globalStartTime = _recordedRoot._start;
    }
    _fullyInitialized = !preliminary;
    declaredAnimation = _parseAnimation(vars.animation);
    if (declaredAnimation && !declaredAnimation.vars.id) {
      declaredAnimation.vars.id = "[no id]";
    }
    _merge3();
    updateList();
    var savedAnimation = _getAnimationById(recall("animation"));
    if (savedAnimation) {
      savedAnimation._inProgress = recall("in") || 0;
      savedAnimation._outProgress = recall("out") || 100;
    }
    vars.paused && pause();
    selectedAnimation = null;
    animation(declaredAnimation || savedAnimation || _recordedRoot);
    var ts = vars.timeScale || recall("timeScale"), savedInOut = savedAnimation === selectedAnimation;
    if (ts) {
      _selectValue(timeScale, ts, timeScaleLabel, ts + "x");
      linkedAnimation.timeScale(ts);
    }
    inProgress = ("inTime" in vars ? _timeToProgress(vars.inTime, selectedAnimation, 0, 0) : savedInOut ? savedAnimation._inProgress : 0) || 0;
    if (inProgress === 100 && !vars.animation && savedAnimation) {
      animation(_recordedRoot);
      inProgress = _timeToProgress(vars.inTime, selectedAnimation, 0, 0) || 0;
    }
    if (inProgress) {
      inPoint.style.left = inProgress + "%";
      inPoint.style.display = outPoint.style.display = "block";
    }
    outProgress = ("outTime" in vars ? _timeToProgress(vars.outTime, selectedAnimation, 100, inProgress) : savedInOut ? savedAnimation._outProgress : 0) || 100;
    if (outProgress < inProgress) {
      outProgress = 100;
    }
    if (outProgress !== 100) {
      outPoint.style.left = outProgress + "%";
      inPoint.style.display = outPoint.style.display = "block";
    }
    loopEnabled = "loop" in vars ? vars.loop : recall("loop");
    loopEnabled && loop(true);
    vars.paused && linkedAnimation.progress(inProgress / 100, true).pause();
    if (_startupPhase && selectedAnimation === _recordedRoot && _globalStartTime && vars.globalSync && !paused) {
      linkedAnimation.time(-_globalStartTime, true);
    }
    updateProgress(true);
  };
  _addListener9(list, "change", onChangeAnimation);
  _addListener9(list, "mousedown", updateList);
  _addListener9(playPauseButton, "mousedown", togglePlayPause);
  _addListener9(find(".seek-bar"), "mousedown", onPressSeekBar);
  _addListener9(find(".rewind"), "mousedown", onPressRewind);
  _addListener9(loopButton, "mousedown", toggleLoop);
  _addListener9(timeScale, "change", onChangeTimeScale);
  if (vars.visibility === "auto") {
    _addListener9(root, "mouseout", onMouseOut);
    _addListener9(root, "mouseover", show);
  } else if (vars.visibility === "hidden") {
    hidden = true;
    autoHideTween.progress(1);
  }
  if (vars.keyboard !== false) {
    if (_keyboardInstance && vars.keyboard) {
      console.warn("[GSDevTools warning] only one instance can be affected by keyboard shortcuts. There is already one active.");
    } else {
      _keyboardInstance = _self;
      keyboardHandler = function keyboardHandler2(e) {
        var key = e.keyCode ? e.keyCode : e.which, ts;
        if (key === 32) {
          togglePlayPause();
        } else if (key === 38) {
          ts = parseFloat(_shiftSelectedValue(timeScale, -1, timeScaleLabel));
          linkedAnimation.timeScale(ts);
          record("timeScale", ts);
        } else if (key === 40) {
          ts = parseFloat(_shiftSelectedValue(timeScale, 1, timeScaleLabel));
          linkedAnimation.timeScale(ts);
          record("timeScale", ts);
        } else if (key === 37) {
          onPressRewind(e);
        } else if (key === 39) {
          linkedAnimation.progress(outProgress / 100);
        } else if (key === 76) {
          toggleLoop();
        } else if (key === 72) {
          toggleHide();
        } else if (key === 73) {
          inProgress = linkedAnimation.progress() * 100;
          record("in", inProgress);
          inPoint.style.left = inProgress + "%";
          updateProgress(true);
        } else if (key === 79) {
          outProgress = linkedAnimation.progress() * 100;
          record("out", outProgress);
          outPoint.style.left = outProgress + "%";
          updateProgress(true);
        }
      };
      _addListener9(_docEl6, "keydown", keyboardHandler);
    }
  }
  gsap27.set(playhead, {
    xPercent: -50,
    x: 0,
    data: "root"
  });
  gsap27.set(inPoint, {
    xPercent: -100,
    x: 0,
    data: "root"
  });
  inPoint._gsIgnore = outPoint._gsIgnore = playhead._gsIgnore = playPauseButton._gsIgnore = loopButton._gsIgnore = true;
  gsap27.killTweensOf([inPoint, outPoint, playhead]);
  initialize(_startupPhase);
  if (_startupPhase) {
    _delayedCall(1e-4, initialize, [false], this);
  }
  gsap27.ticker.add(updateProgress);
  this.update = function(forceMerge) {
    if (_rootInstance === _self) {
      if (!_rootTween.paused() || forceMerge) {
        _merge3();
      }
      updateRootDuration();
    }
  };
  this.kill = this.revert = function() {
    _removeListener9(list, "change", onChangeAnimation);
    _removeListener9(list, "mousedown", updateList);
    _removeListener9(playPauseButton, "mousedown", togglePlayPause);
    _removeListener9(find(".seek-bar"), "mousedown", onPressSeekBar);
    _removeListener9(find(".rewind"), "mousedown", onPressRewind);
    _removeListener9(loopButton, "mousedown", toggleLoop);
    _removeListener9(timeScale, "change", onChangeTimeScale);
    progressDrag.disable();
    inDrag.disable();
    outDrag.disable();
    gsap27.ticker.remove(updateProgress);
    _removeListener9(root, "mouseout", onMouseOut);
    _removeListener9(root, "mouseover", show);
    root.parentNode.removeChild(root);
    if (_rootInstance === _self) {
      _rootInstance = null;
    }
    if (_keyboardInstance === _self) {
      _keyboardInstance = null;
      _removeListener9(_docEl6, "keydown", keyboardHandler);
    }
    delete _lookup3[vars.id + ""];
  };
  this.minimal = function(value) {
    var isMinimal = root.classList.contains("minimal"), p2;
    if (!arguments.length || isMinimal === value) {
      return isMinimal;
    }
    if (value) {
      root.classList.add("minimal");
    } else {
      root.classList.remove("minimal");
    }
    if (vars.container) {
      root.style.top = value ? "calc(100% - 42px)" : "calc(100% - 51px)";
    }
    if (progressDrag.isPressed) {
      skipDragUpdates = true;
      progressDrag.endDrag(progressDrag.pointerEvent);
      skipDragUpdates = false;
      p2 = linkedAnimation.progress() * 100;
      progressBar.style.width = Math.max(0, p2 - inProgress) + "%";
      playhead.style.left = p2 + "%";
      playhead.style.transform = "translate(-50%,0)";
      playhead._gsap.x = "0px";
      playhead._gsap.xPercent = -50;
      progressDrag.startDrag(progressDrag.pointerEvent, true);
    }
  };
  this.animation = animation;
  this.updateList = updateList;
  _context9(this);
};
GSDevTools.version = "3.15.0";
GSDevTools.globalRecordingTime = 2;
GSDevTools.getById = function(id) {
  return id ? _lookup3[id] : _rootInstance;
};
GSDevTools.getByAnimation = function(animation) {
  if (_isString15(animation)) {
    animation = gsap27.getById(animation);
  }
  for (var p2 in _lookup3) {
    if (_lookup3[p2].animation() === animation) {
      return _lookup3[p2];
    }
  }
};
GSDevTools.create = function(vars) {
  return new GSDevTools(vars);
};
GSDevTools.register = _initCore41;
_getGSAP43() && gsap27.registerPlugin(GSDevTools);

// scripts/gsap-entry.mjs
gsapWithCSS.registerPlugin(
  CustomEase,
  CustomWiggle,
  CustomBounce,
  SlowMo,
  ExpoScaleEase,
  RoughEase,
  SplitText,
  MorphSVGPlugin,
  DrawSVGPlugin,
  MotionPathPlugin,
  ScrambleTextPlugin,
  TextPlugin,
  Flip,
  Physics2DPlugin,
  PhysicsPropsPlugin
);
/*! Bundled license information:

gsap/gsap-core.js:
  (*!
   * GSAP 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/CSSPlugin.js:
  (*!
   * CSSPlugin 3.15.0
   * https://gsap.com
   *
   * Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/utils/paths.js:
  (*!
   * paths 3.15.0
   * https://gsap.com
   *
   * Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/CustomEase.js:
  (*!
   * CustomEase 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/CustomWiggle.js:
  (*!
   * CustomWiggle 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/CustomBounce.js:
  (*!
   * CustomBounce 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/EasePack.js:
  (*!
   * EasePack 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/SplitText.js:
  (*!
   * SplitText 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2026, GreenSock. All rights reserved. Subject to the terms at https://gsap.com/standard-license.
   * @author: Jack Doyle
   *)

gsap/MorphSVGPlugin.js:
  (*!
   * MorphSVGPlugin 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/DrawSVGPlugin.js:
  (*!
   * DrawSVGPlugin 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/utils/matrix.js:
  (*!
   * matrix 3.15.0
   * https://gsap.com
   *
   * Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/MotionPathPlugin.js:
  (*!
   * MotionPathPlugin 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/utils/strings.js:
  (*!
   * strings: 3.15.0
   * https://gsap.com
   *
   * Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/ScrambleTextPlugin.js:
  (*!
   * ScrambleTextPlugin 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/TextPlugin.js:
  (*!
   * TextPlugin 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/Flip.js:
  (*!
   * Flip 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/Physics2DPlugin.js:
  (*!
   * Physics2DPlugin 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/PhysicsPropsPlugin.js:
  (*!
   * PhysicsPropsPlugin 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/Observer.js:
  (*!
   * Observer 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/ScrollTrigger.js:
  (*!
   * ScrollTrigger 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/ScrollToPlugin.js:
  (*!
   * ScrollToPlugin 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/Draggable.js:
  (*!
   * Draggable 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
   *)

gsap/utils/VelocityTracker.js:
  (*!
   * VelocityTracker: 3.15.0
   * https://gsap.com
   *
   * Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/InertiaPlugin.js:
  (*!
   * InertiaPlugin 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/ScrollSmoother.js:
  (*!
   * ScrollSmoother 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/PixiPlugin.js:
  (*!
   * PixiPlugin 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/EaselPlugin.js:
  (*!
   * EaselPlugin 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/CSSRulePlugin.js:
  (*!
   * CSSRulePlugin 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/utils/PathEditor.js:
  (*!
   * PathEditor 3.15.0
   * https://gsap.com
   *
   * Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/MotionPathHelper.js:
  (*!
   * MotionPathHelper 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)

gsap/GSDevTools.js:
  (*!
   * GSDevTools 3.15.0
   * https://gsap.com
   *
   * @license Copyright 2008-2026, GreenSock. All rights reserved.
   * Subject to the terms at https://gsap.com/standard-license
   * @author: Jack Doyle, jack@greensock.com
  *)
*/
