// shaders 模块段入口：core 引擎面（createRendererFromJSON）+ js 入口
// （createShader）。typegpu 0.12.3 是唯一运行时依赖，随段内打包。
export * from 'shaders/core';
export * from 'shaders/js';
