```xml
<opencat width="1280" height="720" fps="30" duration="12">
  <template name="stage-slide">
    <div id="$id" class="relative w-[960px] h-[540px] bg-white overflow-hidden $class">
      <div id="$id-grid-x" class="absolute left-[0px] top-[270px] w-[960px] h-[1px] bg-slate-200" />
      <div id="$id-grid-y" class="absolute left-[480px] top-[0px] w-[1px] h-[540px] bg-slate-200" />
      <div id="$id-band" class="absolute right-[0px] top-[0px] w-[286px] h-[540px] $bandTone" />
      <div id="$id-accent" class="absolute left-[72px] top-[72px] w-[84px] h-[8px] rounded-full $accentTone" />
      <text id="$id-kicker" class="absolute left-[72px] top-[92px] text-[20px] font-semibold tracking-[4px] $kickerTone">$kicker</text>
      <text id="$id-title" class="absolute left-[72px] top-[136px] text-[64px] font-bold $titleTone">$title</text>
      <text id="$id-body" class="absolute left-[72px] top-[232px] w-[520px] text-[28px] leading-[1.35] $bodyTone">$body</text>
      <div id="$id-panel" class="absolute right-[72px] bottom-[72px] w-[220px] h-[150px] rounded-[8px] $panelTone overflow-hidden">
        <div id="$id-panel-mark-a" class="absolute left-[18px] top-[22px] w-[132px] h-[10px] rounded-full $panelMarkATone" />
        <div id="$id-panel-mark-b" class="absolute left-[18px] top-[46px] w-[176px] h-[8px] rounded-full $panelMarkBTone" />
        <div id="$id-panel-mark-c" class="absolute left-[18px] top-[66px] w-[120px] h-[8px] rounded-full $panelMarkCTone" />
        <div id="$id-pill" class="absolute left-[18px] bottom-[18px] px-[10px] py-[5px] rounded-full $pillTone">
        <text id="$id-pill-text" class="text-[10px] font-bold $pillTextTone">$pill</text>
        </div>
      </div>
    </div>
  </template>

  <template name="deck-thumb">
    <div id="$id" class="$state flex flex-row gap-[8px]">
      <text id="$id-num" class="w-[16px] text-[11px] font-medium text-right $numTone">$num</text>
      <div id="$id-frame" class="relative w-[144px] h-[81px] rounded-[4px] bg-white overflow-hidden border-[2px] $frameTone">
        <div id="$id-selected" class="absolute inset-0 border-[2px] border-[#D97757] rounded-[3px] opacity-0" />
        <div id="$id-title" class="absolute left-[10px] top-[10px] w-[88px] h-[8px] rounded-full $titleTone" />
        <div id="$id-mark-a" class="absolute left-[10px] bottom-[10px] w-[42px] h-[28px] rounded-[4px] $markATone" />
        <div id="$id-mark-b" class="absolute left-[58px] bottom-[10px] w-[42px] h-[28px] rounded-[4px] $markBTone" />
        <div id="$id-mark-c" class="absolute right-[10px] bottom-[10px] w-[24px] h-[28px] rounded-[4px] $markCTone" />
        <slot name="overlay" />
      </div>
    </div>
  </template>

  <div id="root" class="relative w-[1280px] h-[720px] bg-black overflow-hidden">
    <div id="rail" class="absolute left-[0px] top-[0px] bottom-[0px] w-[188px] bg-[#141414] border-r-[1px] border-[#ffffff14] flex flex-col gap-[12px] px-[10px] py-[12px]">
      <deck-thumb id="thumb-1" state="" num="1" numTone="text-white/55" frameTone="border-[#ffffff00]" titleTone="bg-slate-900" markATone="bg-slate-300" markBTone="bg-slate-200" markCTone="bg-[#D97757]" />

      <deck-thumb id="thumb-2" state="" num="2" numTone="text-white/55" frameTone="border-[#ffffff00]" titleTone="bg-slate-900" markATone="bg-slate-200" markBTone="bg-[#D97757]/80" markCTone="bg-slate-900" />

      <deck-thumb id="thumb-3" state="" num="3" numTone="text-white/55" frameTone="border-[#ffffff00]" titleTone="bg-slate-900" markATone="bg-slate-300" markBTone="bg-slate-400" markCTone="bg-[#D97757]" />

      <deck-thumb id="thumb-4" state="" num="4" numTone="text-white/55" frameTone="border-[#ffffff00]" titleTone="bg-[#D97757]/70" markATone="bg-slate-100 border-[1px] border-slate-200" markBTone="bg-slate-900" markCTone="bg-[#ffffff00]" />
    </div>

    <div id="rail-resize" class="absolute left-[185px] top-[0px] bottom-[0px] w-[6px] bg-[#ffffff00]" />

    <div id="stage" class="absolute left-[188px] top-[0px] right-[0px] bottom-[0px] flex items-center justify-center bg-black">
      <div id="canvas" class="relative w-[960px] h-[540px] overflow-hidden">
        <stage-slide id="slide-1" class="absolute inset-0" bandTone="bg-slate-50" accentTone="bg-[#D97757]" kicker="DECK STAGE" kickerTone="text-[#D97757]" title="Frame shell" titleTone="text-slate-950" body="The authored slide remains inside a fixed design canvas while the player chrome sits above it." bodyTone="text-slate-600" panelTone="bg-slate-100 border-[1px] border-slate-200" panelMarkATone="bg-slate-900" panelMarkBTone="bg-slate-300" panelMarkCTone="bg-slate-300" pill="Layout" pillTone="bg-slate-950" pillTextTone="text-white" />
        <stage-slide id="slide-2" class="absolute inset-0 opacity-0" bandTone="bg-[#fff7ed]" accentTone="bg-[#f59e0b]" kicker="ACTIVE SLIDE" kickerTone="text-[#c2410c]" title="Selected page" titleTone="text-[#431407]" body="The selected thumb and the whole stage page advance together, like a presentation click." bodyTone="text-[#7c2d12]" panelTone="bg-[#ffedd5] border-[1px] border-[#fed7aa]" panelMarkATone="bg-[#431407]" panelMarkBTone="bg-[#fdba74]" panelMarkCTone="bg-[#D97757]" pill="Selected" pillTone="bg-[#D97757]" pillTextTone="text-white" />
        <stage-slide id="slide-3" class="absolute inset-0 opacity-0" bandTone="bg-[#ecfeff]" accentTone="bg-[#0891b2]" kicker="AUTO ADVANCE" kickerTone="text-[#0e7490]" title="Timed flow" titleTone="text-[#164e63]" body="Each four-second beat swaps the stage, updates the slide count, and moves the active thumb." bodyTone="text-[#155e75]" panelTone="bg-[#cffafe] border-[1px] border-[#a5f3fc]" panelMarkATone="bg-[#164e63]" panelMarkBTone="bg-[#67e8f9]" panelMarkCTone="bg-[#0891b2]" pill="Timing" pillTone="bg-[#0891b2]" pillTextTone="text-white" />
      </div>
    </div>

    <div id="overlay" class="absolute left-[542px] bottom-[22px] h-[36px] flex flex-row items-center gap-[4px] px-[4px] py-[4px] rounded-full bg-black text-white">
      <div id="prev-btn" class="w-[28px] h-[28px] rounded-full flex items-center justify-center">
        <icon id="prev-icon" icon="chevron-left" class="w-[14px] h-[14px] stroke-white/75" />
      </div>
      <div id="count-wrap" class="relative w-[64px] h-[28px] flex items-center justify-center">
        <text id="count-1" class="absolute text-[12px] font-medium text-white">1 / 12</text>
        <text id="count-2" class="absolute text-[12px] font-medium text-white opacity-0">2 / 12</text>
        <text id="count-3" class="absolute text-[12px] font-medium text-white opacity-0">3 / 12</text>
      </div>
      <div id="next-btn" class="w-[28px] h-[28px] rounded-full flex items-center justify-center">
        <icon id="next-icon" icon="chevron-right" class="w-[14px] h-[14px] stroke-white/75" />
      </div>
      <div id="divider" class="w-[1px] h-[14px] bg-white/20 ml-[2px] mr-[2px]" />
      <div id="reset-btn" class="h-[28px] rounded-full flex flex-row items-center gap-[6px] px-[10px]">
        <text id="reset-label" class="text-[11px] font-medium text-white/75">Reset</text>
        <text id="reset-key" class="text-[10px] text-white bg-white/10 rounded-[4px] px-[4px] py-[2px]">R</text>
      </div>
    </div>
  </div>

  <script>
    ctx.fromTo('canvas',
      { opacity: 0, scale: 0.985 },
      { opacity: 1, scale: 1, duration: 0.45, ease: 'power2.out' });
    ctx.fromTo('rail',
      { opacity: 0, x: -24 },
      { opacity: 1, x: 0, duration: 0.45, ease: 'power2.out' });
    ctx.fromTo('overlay',
      { opacity: 0, y: 10, scale: 0.94 },
      { opacity: 1, y: 0, scale: 1, duration: 0.35, delay: 0.18, ease: 'power2.out' });

    ctx.fromTo('slide-1',
      { opacity: 0, x: 18, scale: 0.985 },
      { opacity: 1, x: 0, scale: 1, duration: 0.42, delay: 0.28, ease: 'power2.out' });
    ctx.fromTo('thumb-1-selected',
      { opacity: 0 },
      { opacity: 1, duration: 0.18, delay: 0.30, ease: 'power1.out' });

    ctx.to('slide-1',
      { opacity: 0, x: -72, duration: 0.34, delay: 3.72, ease: 'power2.in' });
    ctx.to('thumb-1-selected',
      { opacity: 0, duration: 0.16, delay: 3.78, ease: 'power1.in' });
    ctx.fromTo('slide-2',
      { opacity: 0, x: 86, scale: 0.985 },
      { opacity: 1, x: 0, scale: 1, duration: 0.44, delay: 4.00, ease: 'power2.out' });
    ctx.fromTo('thumb-2-selected',
      { opacity: 0 },
      { opacity: 1, duration: 0.18, delay: 4.08, ease: 'power1.out' });
    ctx.to('count-1',
      { opacity: 0, duration: 0.12, delay: 3.92, ease: 'linear' });
    ctx.fromTo('count-2',
      { opacity: 0 },
      { opacity: 1, duration: 0.12, delay: 4.04, ease: 'linear' });

    ctx.to('slide-2',
      { opacity: 0, x: -72, duration: 0.34, delay: 7.72, ease: 'power2.in' });
    ctx.to('thumb-2-selected',
      { opacity: 0, duration: 0.16, delay: 7.78, ease: 'power1.in' });
    ctx.fromTo('slide-3',
      { opacity: 0, x: 86, scale: 0.985 },
      { opacity: 1, x: 0, scale: 1, duration: 0.44, delay: 8.00, ease: 'power2.out' });
    ctx.fromTo('thumb-3-selected',
      { opacity: 0 },
      { opacity: 1, duration: 0.18, delay: 8.08, ease: 'power1.out' });
    ctx.to('count-2',
      { opacity: 0, duration: 0.12, delay: 7.92, ease: 'linear' });
    ctx.fromTo('count-3',
      { opacity: 0 },
      { opacity: 1, duration: 0.12, delay: 8.04, ease: 'linear' });
  </script>
</opencat>
```