/* Minimal JSX -> h() transform + import/export lowering to CommonJS.
 *
 * Author-facing syntax stays ESM + JSX; this module rewrites it into plain
 * script text that the quickjs / wasm hosts can `new Function(...)` directly:
 *
 *   import { useCanvas } from 'opencat/runtime'
 *     -> const { useCanvas } = __ocRequire('opencat/runtime')
 *   <div class="x">a{b}</div>
 *     -> h("div", { "class": "x" }, "a", b)
 *   export default function App() {}
 *     -> module.exports.default = function App() {}
 *
 * The scanner tracks strings / template literals / comments / regex literals
 * so `<` is only treated as JSX in expression position. It is deliberately
 * small: it covers the dialect this project needs, not all of JavaScript.
 */
(function () {
    'use strict';

    var KEYWORDS_BEFORE_REGEX_OR_JSX = {
        return: true, typeof: true, instanceof: true, in: true, of: true, new: true,
        delete: true, void: true, throw: true, case: true, do: true, else: true,
        yield: true, await: true,
    };

    function quote(value) {
        return JSON.stringify(String(value));
    }

    function TransformState(source) {
        this.src = source;
        this.i = 0;
        this.n = source.length;
        this.prevSig = '';
        this.prevWord = '';
        this.requires = [];
    }

    TransformState.prototype.sig = function (ch) {
        if (/\s/.test(ch)) return;
        this.prevSig = ch;
        if (/[A-Za-z0-9_$]/.test(ch)) this.prevWord += ch;
        else this.prevWord = '';
    };

    // A `/` starts a regex literal (not division) right after these tokens.
    TransformState.prototype.regexAllowed = function () {
        if (this.prevSig === '' || '(,=:[!&|?{;+*%<>~^'.indexOf(this.prevSig) >= 0) return true;
        return Object.prototype.hasOwnProperty.call(KEYWORDS_BEFORE_REGEX_OR_JSX, this.prevWord);
    };

    // A `<` starts JSX in the same positions a regex literal would be legal.
    TransformState.prototype.jsxAllowed = function () {
        if (!this.regexAllowed()) return false;
        var next = this.src[this.i + 1];
        return next === '>' || /[A-Za-z_$]/.test(next);
    };

    TransformState.prototype.skipWs = function () {
        while (this.i < this.n) {
            var ch = this.src[this.i];
            if (/\s/.test(ch)) { this.i++; continue; }
            if (ch === '/' && this.src[this.i + 1] === '/') {
                while (this.i < this.n && this.src[this.i] !== '\n') this.i++;
                continue;
            }
            if (ch === '/' && this.src[this.i + 1] === '*') {
                this.i += 2;
                while (this.i < this.n && !(this.src[this.i] === '*' && this.src[this.i + 1] === '/')) this.i++;
                this.i = Math.min(this.n, this.i + 2);
                continue;
            }
            return;
        }
    };

    TransformState.prototype.readStringLiteral = function () {
        var start = this.i;
        var quoteCh = this.src[this.i];
        this.i++;
        while (this.i < this.n) {
            var ch = this.src[this.i];
            if (ch === '\\') { this.i += 2; continue; }
            this.i++;
            if (ch === quoteCh) break;
        }
        return this.src.slice(start, this.i);
    };

    TransformState.prototype.readTemplate = function () {
        var start = this.i;
        this.i++; // opening backtick
        while (this.i < this.n) {
            var ch = this.src[this.i];
            if (ch === '\\') { this.i += 2; continue; }
            if (ch === '`') { this.i++; break; }
            if (ch === '$' && this.src[this.i + 1] === '{') {
                this.i += 2;
                var depth = 1;
                while (this.i < this.n && depth > 0) {
                    var inner = this.src[this.i];
                    if (inner === '{') depth++;
                    else if (inner === '}') depth--;
                    else if (inner === '"' || inner === "'") { this.readStringLiteral(); continue; }
                    else if (inner === '`') { this.readTemplate(); continue; }
                    this.i++;
                }
                continue;
            }
            this.i++;
        }
        return this.src.slice(start, this.i);
    };

    TransformState.prototype.readRegex = function () {
        var start = this.i;
        this.i++; // opening slash
        var inClass = false;
        while (this.i < this.n) {
            var ch = this.src[this.i];
            if (ch === '\\') { this.i += 2; continue; }
            if (ch === '[') inClass = true;
            else if (ch === ']') inClass = false;
            else if (ch === '/' && !inClass) { this.i++; break; }
            else if (ch === '\n') break;
            this.i++;
        }
        while (this.i < this.n && /[a-z]/.test(this.src[this.i])) this.i++; // flags
        return this.src.slice(start, this.i);
    };

    // Reads a balanced `{ ... }` expression container (opening brace already at
    // this.i) and returns the TRANSFORMED inner text without the braces.
    TransformState.prototype.readBraced = function () {
        this.i++; // consume '{'
        return this.scanCode(true);
    };

    TransformState.prototype.readName = function () {
        var start = this.i;
        while (this.i < this.n && /[A-Za-z0-9_$.\-]/.test(this.src[this.i])) this.i++;
        if (this.i === start) throw new Error('use: expected a JSX tag name');
        return this.src.slice(start, this.i);
    };

    TransformState.prototype.readAttrName = function () {
        var start = this.i;
        while (this.i < this.n && /[A-Za-z0-9_$\-:]/.test(this.src[this.i])) this.i++;
        if (this.i === start) throw new Error('use: expected a JSX attribute name');
        return this.src.slice(start, this.i);
    };

    TransformState.prototype.skipClosingTag = function () {
        // this.i points at '<' of '</...>'
        this.i += 2;
        while (this.i < this.n && this.src[this.i] !== '>') this.i++;
        if (this.i < this.n) this.i++;
    };

    // Scans ordinary JS code, lowering any JSX found in expression position.
    // When inBraces is true, stops (consuming the '}') at the matching close.
    TransformState.prototype.scanCode = function (inBraces) {
        var out = '';
        var depth = 0;
        while (this.i < this.n) {
            var ch = this.src[this.i];
            if (inBraces && depth === 0 && ch === '}') { this.i++; return out; }
            if (ch === '/' && this.src[this.i + 1] === '/') {
                var lineEnd = this.i;
                while (lineEnd < this.n && this.src[lineEnd] !== '\n') lineEnd++;
                out += this.src.slice(this.i, lineEnd);
                this.i = lineEnd;
                continue;
            }
            if (ch === '/' && this.src[this.i + 1] === '*') {
                var blockEnd = this.i + 2;
                while (blockEnd < this.n && !(this.src[blockEnd] === '*' && this.src[blockEnd + 1] === '/')) blockEnd++;
                blockEnd = Math.min(this.n, blockEnd + 2);
                out += this.src.slice(this.i, blockEnd);
                this.i = blockEnd;
                continue;
            }
            if (ch === '"' || ch === "'") {
                out += this.readStringLiteral();
                this.prevSig = ch;
                this.prevWord = '';
                continue;
            }
            if (ch === '`') {
                out += this.readTemplate();
                this.prevSig = '`';
                this.prevWord = '';
                continue;
            }
            if (ch === '/' && this.regexAllowed()) {
                out += this.readRegex();
                this.prevSig = '/';
                this.prevWord = '';
                continue;
            }
            if (ch === '<' && this.jsxAllowed()) {
                out += this.parseElement();
                this.prevSig = ')';
                this.prevWord = '';
                continue;
            }
            if (ch === '{') depth++;
            else if (ch === '}') depth--;
            out += ch;
            this.sig(ch);
            this.i++;
        }
        if (inBraces) throw new Error('use: unterminated `{ ... }` expression');
        return out;
    };

    // this.i points at '<' of an element. Returns the h(...) call text.
    TransformState.prototype.parseElement = function () {
        this.i++; // '<'
        var name;
        if (this.src[this.i] === '>') {
            name = '__ocFragment';
            this.i++;
        } else {
            name = this.readName();
        }

        var props = [];
        var selfClosing = false;
        var isFragment = name === '__ocFragment';
        for (;;) {
            this.skipWs();
            var ch = this.src[this.i];
            if (ch === undefined) throw new Error('use: unterminated JSX element <' + name + '>');
            if (isFragment && ch === '<') break; // `<>` has no attributes; child starts
            if (ch === '/' && this.src[this.i + 1] === '>') { this.i += 2; selfClosing = true; break; }
            if (ch === '>') { this.i++; break; }
            if (ch === '{') {
                var spread = this.readBraced().trim();
                if (spread.slice(0, 3) === '...') props.push(spread);
                else props.push('...(' + spread + ')');
                continue;
            }
            var attrName = this.readAttrName();
            this.skipWs();
            if (this.src[this.i] === '=') {
                this.i++;
                this.skipWs();
                var valueCh = this.src[this.i];
                if (valueCh === '"' || valueCh === "'") {
                    props.push(quote(attrName) + ': ' + this.readStringLiteral());
                } else if (valueCh === '{') {
                    props.push(quote(attrName) + ': ' + this.readBraced());
                } else {
                    throw new Error('use: unsupported JSX attribute value for "' + attrName + '"');
                }
            } else {
                props.push(quote(attrName) + ': true');
            }
        }

        var children = [];
        if (!selfClosing) {
            while (this.i < this.n) {
                if (this.src[this.i] === '<' && this.src[this.i + 1] === '/') {
                    this.skipClosingTag();
                    break;
                }
                if (this.src[this.i] === '<') { children.push(this.parseElement()); continue; }
                if (this.src[this.i] === '{') {
                    var expr = this.readBraced().trim();
                    if (expr !== '' && !/^\/\*[\s\S]*\*\/$/.test(expr)) children.push(expr);
                    continue;
                }
                var textStart = this.i;
                while (this.i < this.n && this.src[this.i] !== '<' && this.src[this.i] !== '{') this.i++;
                var text = this.src.slice(textStart, this.i);
                if (text.trim() !== '') children.push(quote(text.trim()));
            }
        }

        var tag;
        if (name === '__ocFragment') {
            tag = 'globalThis.__ocFragment';
        } else if (/^[A-Z]/.test(name) || name.indexOf('.') >= 0) {
            // Component reference (capitalized or member expression): pass the
            // function itself, like React's classic runtime.
            tag = name;
        } else {
            tag = quote(name);
        }
        var args = [tag, '{' + props.join(', ') + '}'].concat(children);
        return 'h(' + args.join(', ') + ')';
    };

    // ── import / export lowering ─────────────────────────────────────
    var IMPORT_FROM = /^\s*import\s+([\s\S]*?)\s+from\s*(['"])([^'"]+)\2\s*;?\s*$/;
    var IMPORT_BARE = /^\s*import\s*(['"])([^'"]+)\1\s*;?\s*$/;

    function stripImports(source, requires) {
        var prelude = [];
        var rest = [];
        var lines = source.split('\n');
        for (var li = 0; li < lines.length; li++) {
            var line = lines[li];
            var bare = IMPORT_BARE.exec(line);
            if (bare) {
                requires.push(bare[2]);
                prelude.push('__ocRequire(' + quote(bare[2]) + ');');
                continue;
            }
            var match = IMPORT_FROM.exec(line);
            if (match) {
                var clause = match[1].trim();
                var spec = match[3];
                requires.push(spec);
                var decls = [];
                var star = /\*\s+as\s+([A-Za-z_$][\w$]*)/.exec(clause);
                if (star) decls.push('const ' + star[1] + ' = __ocRequire(' + quote(spec) + ');');
                clause = clause.replace(/\*\s+as\s+[A-Za-z_$][\w$]*/, '').replace(/^,/, '').replace(/,$/, '').trim();
                var named = /\{\s*([\s\S]*?)\s*\}/.exec(clause);
                if (named) {
                    var bindings = named[1].split(',').map(function (part) {
                        return part.trim();
                    }).filter(function (part) { return part !== ''; }).map(function (part) {
                        var asMatch = /^([A-Za-z_$][\w$]*)\s+as\s+([A-Za-z_$][\w$]*)$/.exec(part);
                        return asMatch ? asMatch[1] + ': ' + asMatch[2] : part;
                    });
                    decls.push('const { ' + bindings.join(', ') + ' } = __ocRequire(' + quote(spec) + ');');
                    clause = clause.replace(/\{\s*[\s\S]*?\s*\}/, '').replace(/^,/, '').replace(/,$/, '').trim();
                }
                if (clause !== '') {
                    decls.push('const ' + clause + ' = __ocRequire(' + quote(spec) + ').default;');
                }
                for (var d = 0; d < decls.length; d++) prelude.push(decls[d]);
                continue;
            }
            rest.push(line);
        }
        return { prelude: prelude.join('\n'), code: rest.join('\n'), requires: requires };
    }

    var EXPORT_DEFAULT = /(^|\n)([ \t]*)export\s+default\s+/g;
    var EXPORT_NAMED = /(^|\n)([ \t]*)export\s+(?=(const|let|var|function|class)\b)/g;

    function lowerExports(code) {
        var names = [];
        var namedRe = /(^|\n)[ \t]*export\s+(?:const|let|var|function|class)\s+([A-Za-z_$][\w$]*)/g;
        var m;
        while ((m = namedRe.exec(code)) !== null) names.push(m[2]);
        var out = code.replace(EXPORT_DEFAULT, function (all, lead, indent) {
            return lead + indent + 'module.exports.default = ';
        });
        out = out.replace(EXPORT_NAMED, function (all, lead, indent) {
            return lead + indent;
        });
        var trailer = '';
        for (var i = 0; i < names.length; i++) {
            trailer += 'module.exports.' + names[i] + ' = ' + names[i] + ';\n';
        }
        return { code: prelude_join(out, trailer), exports: names };
    }

    function prelude_join(code, trailer) {
        return code + (trailer === '' ? '' : '\n' + trailer);
    }

    function transform(source) {
        var requires = [];
        var stripped = stripImports(source, requires);
        var state = new TransformState(stripped.code);
        var jsxCode = state.scanCode(false);
        var lowered = lowerExports(jsxCode);
        var head = stripped.prelude === '' ? '' : stripped.prelude + '\n';
        return {
            code: head + lowered.code,
            requires: requires,
            exports: lowered.exports,
        };
    }

    globalThis.__ocTransformJsx = transform;
})();
