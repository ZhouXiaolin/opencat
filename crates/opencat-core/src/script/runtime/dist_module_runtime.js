/* Dist module runtime for HTML/JS documents.
 *
 * core is a deterministic video renderer, not a browser engine. This file
 * only supplies the small module table that compiled dist pages need:
 * ESM imports lower to __ocRequire(...), exports lower to module.exports.
 * It does not provide a JSX factory, React hooks, components, or a DOM tree.
 *
 * Frames stay pure: frame = f(t), animate = g(t). The page driver rebuilds
 * the HTML seed tree and replays scripts at the requested frame time.
 */
(function () {
    'use strict';
    if (globalThis.__ocDistModules) return;

    var builtins = Object.create(null);
    var modules = Object.create(null);
    var factories = Object.create(null);
    var factorySource = Object.create(null);
    var currentModule = null;

    function resolveSpec(spec) {
        if (Object.prototype.hasOwnProperty.call(builtins, spec)) return spec;
        var sources = globalThis.__ocModuleSources || {};
        if (Object.prototype.hasOwnProperty.call(sources, spec)) return spec;
        if (spec.charAt(0) !== '.') {
            throw new Error('dist module: unknown package "' + spec + '"; bundle it into HTML/JS dist first');
        }
        var base = currentModule || '';
        var slash = base.lastIndexOf('/');
        var parts = slash >= 0 ? base.slice(0, slash).split('/') : [];
        var segments = spec.split('/');
        for (var i = 0; i < segments.length; i++) {
            var seg = segments[i];
            if (seg === '' || seg === '.') continue;
            if (seg === '..') parts.pop();
            else parts.push(seg);
        }
        var joined = parts.join('/');
        if (Object.prototype.hasOwnProperty.call(sources, joined)) return joined;
        var suffixes = ['.js', '/index.js'];
        for (var s = 0; s < suffixes.length; s++) {
            if (Object.prototype.hasOwnProperty.call(sources, joined + suffixes[s])) {
                return joined + suffixes[s];
            }
        }
        throw new Error('dist module: cannot resolve "' + spec + '" from "' + (base || '<entry>') + '"');
    }

    globalThis.__ocReexportAll = function (exports, moduleExports) {
        if (!moduleExports) return;
        Object.keys(moduleExports).forEach(function (key) {
            if (key !== 'default' && exports[key] === undefined) exports[key] = moduleExports[key];
        });
    };

    function requireModule(spec) {
        if (spec !== 'gsap' && spec.indexOf('gsap/') === 0) {
            requireModule('gsap');
            var subpath = builtins[spec];
            if (!subpath) throw new Error('dist module: gsap has no subpath "' + spec + '"');
            return subpath;
        }
        var key = resolveSpec(spec);
        if (Object.prototype.hasOwnProperty.call(builtins, key)) return builtins[key];
        if (Object.prototype.hasOwnProperty.call(modules, key)) return modules[key].exports;
        var sources = globalThis.__ocModuleSources || {};
        if (!Object.prototype.hasOwnProperty.call(sources, key)) {
            throw new Error('dist module: "' + key + '" was not provided by the host');
        }
        var mod = { exports: {} };
        modules[key] = mod;
        var previous = currentModule;
        currentModule = key;
        try {
            var source = sources[key];
            var factory = factories[key];
            if (factory === undefined || factorySource[key] !== source) {
                factory = new Function('__ocRequire', 'require', 'module', 'exports', 'document', 'window', source);
                factories[key] = factory;
                factorySource[key] = source;
            }
            factory(
                requireModule,
                requireModule,
                mod,
                mod.exports,
                globalThis.__ocDocument || globalThis.document,
                globalThis.__ocWindow || globalThis.window
            );
        } finally {
            currentModule = previous;
        }
        return mod.exports;
    }

    globalThis.__ocRequire = requireModule;
    globalThis.__ocResetDistModules = function () {
        var kept = {};
        for (var lib in { gsap: 1, three: 1, 'canvaskit-wasm': 1, g2: 1 }) {
            if (modules[lib]) kept[lib] = modules[lib];
        }
        modules = Object.create(null);
        for (lib in kept) modules[lib] = kept[lib];
    };
    globalThis.__ocDistModules = true;
})();
