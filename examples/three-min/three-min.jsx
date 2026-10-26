// 批次③：官方 three 包（npm three 0.181）在引擎 realm 里整体执行，
// WebGL 调用走 core 的 webgl_api.js 录制面 → GpuSection → engine 直调执行。
import { useEffect, useGSAP, useRef } from 'opencat/runtime';
import * as THREE from 'three';

export const meta = { width: 640, height: 360, fps: 10, duration: 1.0 };

function ThreeMin() {
  const canvas = useRef(null);
  useGSAP(() => {});
  useEffect(() => {
    var g = ctx.current.__threeMin;
    if (!g) {
      var renderer = new THREE.WebGLRenderer({ canvas: canvas.current, antialias: true });
      renderer.setSize(640, 360);
      renderer.setPixelRatio(1);
      renderer.setClearColor(0x101820, 1);

      var scene = new THREE.Scene();
      var camera = new THREE.PerspectiveCamera(45, 640 / 360, 0.1, 100);
      camera.position.set(0, 0, 4);

      var box = new THREE.Mesh(
        new THREE.BoxGeometry(1.4, 1.4, 1.4),
        new THREE.MeshStandardMaterial({ color: 0xd7ff3f, roughness: 0.35, metalness: 0.15 })
      );
      scene.add(box);
      scene.add(new THREE.AmbientLight(0xffffff, 0.5));
      var key = new THREE.DirectionalLight(0xffffff, 2.0);
      key.position.set(2, 3, 4);
      scene.add(key);

      g = ctx.current.__threeMin = { scene, camera, renderer, box };
    }
    // 每帧旋转让 10 帧各不相同
    var t = ctx.current.time;
    g.box.rotation.x = Math.PI * t;
    g.box.rotation.y = Math.PI * 2 * t;
    g.camera.lookAt(0, 0, 0);
    g.renderer.render(g.scene, g.camera);
  });

  return (
    <div class="relative w-[640px] h-[360px]">
      <canvas id="glcanvas" ref={canvas} class="absolute left-0 top-0 w-[640px] h-[360px]" />
    </div>
  );
}

export default function Main() {
  return <ThreeMin />;
}
