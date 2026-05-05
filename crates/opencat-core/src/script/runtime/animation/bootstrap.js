(function() {
    var runtime = {};

    runtime.SPRING_PRESETS = {
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
        plugins: [],
        specialOptions: {},
        timingKeys: {
            duration: true,
            delay: true,
            ease: true,
            easing: true,
            clamp: true,
            repeat: true,
            yoyo: true,
            repeatDelay: true,
            stagger: true,
            __skipSceneFit: true,
            keyframes: true,
            at: true,
        },

        registerSpecialOption: function(name) {
            this.specialOptions[name] = true;
        },

        isTimingKey: function(name) {
            return !!this.timingKeys[name];
        },

        isReservedKey: function(name) {
            return this.isTimingKey(name) || !!this.specialOptions[name];
        },

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

        registerPlugin: function(plugin) {
            if (!plugin || typeof plugin !== 'object') {
                throw new Error('animation plugin must be an object');
            }
            var name = plugin.name || '<anonymous>';
            if (plugin.specialOptions) {
                for (var s = 0; s < plugin.specialOptions.length; s++) {
                    this.registerSpecialOption(plugin.specialOptions[s]);
                }
            }
            var properties = plugin.properties || {};
            for (var propName in properties) {
                if (runtime.hasOwn(properties, propName)) {
                    this.registerProperty(propName, properties[propName], name);
                }
            }
            this.plugins.push(plugin);
            if (typeof plugin.install === 'function') {
                plugin.install(this, runtime.core);
            }
            return plugin;
        },
    };

    ctx.registerPlugin = function(plugin) {
        runtime.animation.registerPlugin(plugin);
        return ctx;
    };

    ctx.animation = {
        registerPlugin: function(plugin) {
            runtime.animation.registerPlugin(plugin);
            return ctx.animation;
        },
        plugins: function() {
            return runtime.animation.plugins.map(function(plugin) {
                return plugin.name || '<anonymous>';
            });
        },
    };

    globalThis.__opencatAnimation = runtime;
})();
