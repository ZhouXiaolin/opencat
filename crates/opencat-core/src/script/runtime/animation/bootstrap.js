// 动画属性注册表：ocTarget 桥（ctx.getNode(id).prop(v) 的 descriptor 转
// 发）的唯一支撑。补间引擎与 ctx.* 方言面已退役——作者动画一律走官方
// gsap（项目侧 bundle 的 gsap 模块段供货）。
(function() {
    var runtime = {};

    runtime.hasOwn = function(obj, key) {
        return Object.prototype.hasOwnProperty.call(obj, key);
    };

    runtime.copyOwn = function(base, extra) {
        var out = {};
        base = base || {};
        extra = extra || {};
        for (var k in base) {
            if (runtime.hasOwn(base, k)) out[k] = base[k];
        }
        for (var e in extra) {
            if (runtime.hasOwn(extra, e)) out[e] = extra[e];
        }
        return out;
    };

    runtime.animation = {
        properties: {},
        aliases: {},

        registerProperty: function(name, descriptor, pluginName) {
            if (!name || typeof name !== 'string') {
                throw new Error('animation property name must be a non-empty string');
            }
            if (this.properties[name] || this.aliases[name]) {
                throw new Error('animation property already registered `' + name + '`');
            }
            descriptor = descriptor || {};
            descriptor.name = name;
            descriptor.pluginName = pluginName || descriptor.pluginName || '<anonymous>';
            this.properties[name] = descriptor;

            var aliases = descriptor.aliases || [];
            for (var i = 0; i < aliases.length; i++) {
                var alias = aliases[i];
                if (this.properties[alias] || this.aliases[alias]) {
                    throw new Error('animation property alias conflict `' + alias + '`');
                }
                this.aliases[alias] = name;
            }
        },

        resolveProperty: function(name) {
            return this.properties[name] || this.properties[this.aliases[name]];
        },

        canonicalName: function(name) {
            return this.properties[name] ? name : this.aliases[name];
        },
    };

    globalThis.__opencatAnimation = runtime;
})();
