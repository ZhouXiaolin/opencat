// ocTarget 桥的支撑面（补间引擎已退役——作者动画一律走官方 gsap）。
// 这三个函数只服务 jsx_runtime.js 的 ctx.ocTarget 描述符转发与
// runtime/text_units.js 的文字拆分，不是 gsap 方言：goal.md §5 明确
// 「生命周期与桥接服务不退役」。
(function() {
    var runtime = globalThis.__opencatAnimation;
    var hasOwn = runtime.hasOwn;

    function normalizeSelector(target) {
        if (typeof target !== 'string') {
            return target;
        }
        return target.charAt(0) === '#' ? target.slice(1) : target;
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

    runtime.core = {
        normalizeSelector: normalizeSelector,
        createTarget: createTarget,
        getDescriptorDefault: getDescriptorDefault,
    };
})();
