// react → opencat 垫片。opencat-plugin.mjs 让 esbuild 把 `react` 与
// `react/jsx-runtime` alias 到本文件：npm 组件（lucide-react 等）按 React
// 语义创建元素，全部照常物化为引擎 h() 节点。`<svg>` 边界把子树序列化成
// markup 字符串，交给引擎的 `<svg inline>` 通道（usvg 直接吃字符串）。
// svg 内部节点不进布局/CSS/动画——width/height/class 挂在 svg 根上，与
// 真实 HTML 工程一致。
//
// 注意 `<path>` 双重身份：opencat 顶层 `<path>`（带 d/view-box）照常走
// 引擎 Path 元素；只有 svg 子树内部的 path 才被序列化——按"是否存在 svg
// 祖先"分流，无需标签名单。
//
// 限制：`currentColor` 解析走 SVG 原生语义——`color` 是可继承表示属性，
// lucide 的 color prop 会以 color="…" 落在根标签上；CSS class 不会级联进
// svg 内部（usvg 不吃样式表）。
import { h, Fragment, useRef } from 'opencat/runtime';

function escText(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

function escAttr(s) {
  return escText(s).replace(/"/g, '&quot;');
}

function kebabAttr(name) {
  return name.replace(/[A-Z]/g, (c) => '-' + c.toLowerCase());
}

// SVG 原生 camelCase 属性（React 原样透传），不能转 kebab-case。
const NATIVE_CAMEL_ATTRS = new Set([
  'viewBox', 'preserveAspectRatio',
  'gradientTransform', 'gradientUnits', 'spreadMethod',
  'patternTransform', 'patternUnits', 'patternContentUnits',
  'clipPathUnits', 'maskUnits', 'maskContentUnits',
  'filterUnits', 'primitiveUnits', 'stdDeviation', 'numOctaves',
  'baseFrequency', 'tableValues', 'textLength', 'startOffset',
  'specularConstant', 'specularExponent', 'surfaceScale', 'diffuseConstant',
  'kernelMatrix', 'limitingConeAngle', 'pointsAtX', 'pointsAtY', 'pointsAtZ',
  'markerUnits', 'markerWidth', 'markerHeight', 'refX', 'refY',
]);

function svgAttrName(name) {
  if (name.indexOf('-') >= 0 || NATIVE_CAMEL_ATTRS.has(name)) return name;
  return kebabAttr(name);
}

function serializeStyle(style) {
  if (!style || typeof style !== 'object') return '';
  var decls = [];
  for (var k in style) {
    if (Object.prototype.hasOwnProperty.call(style, k) && style[k] != null) {
      decls.push(svgAttrName(k) + ':' + String(style[k]));
    }
  }
  return decls.join(';');
}

// props → ` k="v"` 属性串。skip 集合排除引擎按 props 消费（不进 markup）
// 与 React 内部注入的键。
function serializeAttrs(props, skip) {
  var attrs = '';
  for (var key in props) {
    if (!Object.prototype.hasOwnProperty.call(props, key)) continue;
    if (skip[key]) continue;
    var v = props[key];
    if (v === undefined || v === null || v === false || typeof v === 'function') continue;
    if (key === 'style') {
      var css = serializeStyle(v);
      if (css) attrs += ' style="' + escAttr(css) + '"';
      continue;
    }
    if (v === true) {
      attrs += ' ' + svgAttrName(key);
      continue;
    }
    attrs += ' ' + svgAttrName(key) + '="' + escAttr(String(v)) + '"';
  }
  return attrs;
}

var ROOT_SKIP = {
  children: true, key: true, ref: true,
  className: true, class: true, id: true,
  width: true, height: true, preserveAspectRatio: true,
};
var INNER_SKIP = { children: true, key: true, ref: true };

function serializeNode(node) {
  if (node === null || node === undefined || node === false || node === true) return '';
  if (typeof node === 'string' || typeof node === 'number') return escText(node);
  if (Array.isArray(node)) {
    var out = '';
    for (var i = 0; i < node.length; i++) out += serializeNode(node[i]);
    return out;
  }
  if (node && node.__ocEl === true) {
    if (typeof node.tag === 'function') {
      // svg 内部的组件包装（<svg><MyGlyph/></svg）：就地调用并序列化结果。
      return serializeNode(node.tag(node.props));
    }
    if (node.tag === Fragment) return serializeNode(node.children);
    var tag = node.tag;
    var props = node.props || {};
    return '<' + tag + serializeAttrs(props, INNER_SKIP) + '>' +
      serializeNode(node.children) + '</' + tag + '>';
  }
  // 其余值（普通 object 等）在 svg 子树里没有意义：明确报错。
  throw new Error(
    'react-shim: unsupported value inside an <svg> subtree (' + typeof node + ')'
  );
}

// `<svg>` 根：children 序列化成完整文档（根标签属性进 markup，usvg 从中取
// intrinsic 尺寸与 color 继承链）；className/width/height/style 照常留在
// props 上——引擎按普通 svg 元素处理布局与 CSS。`src` 是文件模式，不序列化。
function materializeSvg(props) {
  if (props.src !== undefined && props.src !== null) {
    return h('svg', props);
  }
  var markup =
    '<svg xmlns="http://www.w3.org/2000/svg"' +
    serializeAttrs(props, ROOT_SKIP) +
    '>' +
    serializeNode(props.children) +
    '</svg>';
  return h('svg', { inline: markup, ...props });
}

function createElement(type, config) {
  var props = {};
  if (config) {
    for (var k in config) {
      if (k === 'key' || k === '__source' || k === '__self') continue;
      props[k] = config[k];
    }
  }
  var args = [];
  for (var i = 2; i < arguments.length; i++) args.push(arguments[i]);
  // React children 语义：单 child 原样、多 child 数组，引擎 h() 会再做
  // 一次扁平化与无效值过滤，这里传原始形态即可。
  if (typeof props.children === 'undefined' && args.length === 1) props.children = args[0];
  else if (args.length > 1) props.children = args;

  if (typeof type === 'function') {
    return h(type, props);
  }
  if (type === 'svg') {
    return materializeSvg(props);
  }
  if (type === Fragment) {
    return h(Fragment, props, props.children);
  }
  return h(type, props, props.children);
}

// automatic runtime：esbuild 生成 `import { jsx, jsxs } from "react/jsx-runtime"`。
// key 与引擎无关（没有 reconciliation），直接忽略。
function jsx(type, props) {
  return createElement(type, props || {});
}

const jsxs = jsx;
const jsxDEV = jsx;

// lucide-react 用 forwardRef 包图标组件；引擎按普通函数组件调用
// （el.tag(props)），ref 透传无意义，直接解包。
function forwardRef(fn) {
  fn.displayName = fn.displayName || fn.name;
  return fn;
}

function memo(fn) {
  return fn;
}

export {
  createElement,
  createElement as h,
  jsx,
  jsxs,
  jsxDEV,
  Fragment,
  forwardRef,
  memo,
  useRef,
};
export default {
  createElement,
  jsx,
  jsxs,
  jsxDEV,
  Fragment,
  forwardRef,
  memo,
  useRef,
};
