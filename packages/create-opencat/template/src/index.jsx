import { Zap } from 'lucide-react';
import { useGSAP } from 'opencat/runtime';

// 尺寸 / 帧率 / 时长只在这里声明，opencat render 不接受命令行覆盖。
export const meta = { width: 1920, height: 1080, fps: 30, duration: 3 };

// 元素只描述静止构图；动效全部写进 useGSAP。
// npm 组件（lucide-react 等）在 vite build 时被打进 bundle：图标按真正的
// HTML 工程那样编译成内联 svg，color prop 沿 SVG 语义给 currentColor 上色。
export default function Composition() {
  useGSAP((ctx) => {
    ctx.from('icon', { opacity: 0, scale: 0.5, duration: 0.5, ease: 'back.out(2)' });
    ctx.from('badge', { opacity: 0, y: 24, duration: 0.6, ease: 'power2.out' });
    ctx.from('title', { opacity: 0, y: 48, duration: 0.8, delay: 0.2, ease: 'power3.out' });
    ctx.from('subtitle', { opacity: 0, y: 24, duration: 0.8, delay: 0.45, ease: 'power3.out' });
  });

  return (
    <div id="root" class="relative w-[1920px] h-[1080px] overflow-hidden bg-slate-950">
      <div id="glow" class="absolute left-[560px] top-[240px] w-[800px] h-[600px] rounded-full bg-indigo-600/30 blur-[120px]" />
      <Zap id="icon" size={72} strokeWidth={2.5} color="#fbbf24" class="absolute left-[924px] top-[300px]" />
      <text id="badge" class="absolute left-0 top-[420px] w-[1920px] text-center text-[30px] font-medium tracking-[0.3em] text-indigo-300 uppercase">Vite Template</text>
      <text id="title" class="absolute left-0 top-[490px] w-[1920px] text-center text-[110px] font-bold text-white">Hello OpenCat</text>
      <text id="subtitle" class="absolute left-0 top-[670px] w-[1920px] text-center text-[36px] text-slate-400">npm run build → dist/video.mp4</text>
    </div>
  );
}
