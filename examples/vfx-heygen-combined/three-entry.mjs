// three 真包入口：主包全量导出(scripts/build-example-three.mjs 与
// vite 插件的 bundleThree 共用)。GLTFLoader/DRACOLoader 从官方
// examples/jsm 子路径折叠进同一 bundle(它们 import 'three' 落回主包),
// 随命名 re-export——jsx 侧 `import { GLTFLoader } from 'three'` 取官方
// ESM 拼法。
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { DRACOLoader } from 'three/examples/jsm/loaders/DRACOLoader.js';

export * from 'three';
export { GLTFLoader, DRACOLoader };
