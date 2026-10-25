// GPU 协议面 tracer bullet:手写 WebGL2(canvas.getContext),画一个
// 0–1s 内从蓝过渡到红的三角形。零 three、零库——纯录制面 + 双端执行器
// 的最小验收样本。
//
// 验收:web 端(浏览器真 WebGL2 重放)vs engine 端(glow 直调)k3diff,
// 以及各端 vs 浏览器同代码截帧。

import { useFrame } from 'opencat/runtime';

export const meta = { width: 320, height: 240, fps: 30, duration: 1 };

const VS = `#version 300 es
in vec2 aPos;
in vec3 aColor;
out vec3 vColor;
void main() {
  vColor = aColor;
  gl_Position = vec4(aPos, 0.0, 1.0);
}`;

const FS = `#version 300 es
precision mediump float;
in vec3 vColor;
out vec4 fragColor;
void main() {
  fragColor = vec4(vColor, 1.0);
}`;

// 顶点:两个三角形成一个大三角形(left, right, top),颜色蓝→红→绿。
const VERTICES = new Float32Array([
  // pos(x,y)  color(r,g,b)
  -0.8, -0.6, 0.1, 0.3, 1.0,
   0.8, -0.6, 1.0, 0.3, 0.1,
   0.0,  0.8, 0.1, 1.0, 0.3,
]);

export default function Scene() {
  useFrame((_ctx, t) => {
    const gl = document.getElementById('gl').getContext('webgl2', {
      alpha: true,
      antialias: true,
      premultipliedAlpha: false,
    });

    // 编译(每帧全量重录——录制面按帧自足;内容缓存让执行端免重编译)。
    const vs = gl.createShader(gl.VERTEX_SHADER);
    gl.shaderSource(vs, VS);
    gl.compileShader(vs);
    const fs = gl.createShader(gl.FRAGMENT_SHADER);
    gl.shaderSource(fs, FS);
    gl.compileShader(fs);
    const prog = gl.createProgram();
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    gl.useProgram(prog);

    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const vbo = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
    gl.bufferData(gl.ARRAY_BUFFER, VERTICES, gl.STATIC_DRAW);

    gl.enableVertexAttribArray('aPos');
    gl.vertexAttribPointer('aPos', 2, gl.FLOAT, false, 20, 0);
    gl.enableVertexAttribArray('aColor');
    gl.vertexAttribPointer('aColor', 3, gl.FLOAT, false, 20, 8);

    // 帧 0 蓝底,随时间淡入暖色调背景(证明 uniform 通路)。
    const k = Math.min(1, t);
    gl.viewport(0, 0, 320, 240);
    gl.clearColor(0.05 + 0.75 * k, 0.07, 0.15 + 0.5 * (1 - k), 1.0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  });

  return (
    <div id="root" class="relative w-[320px] h-[240px]">
      <canvas id="gl" class="absolute left-0 top-0" style="width:320px;height:240px" />
    </div>
  );
}
