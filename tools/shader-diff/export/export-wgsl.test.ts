// zz-export-wgsl.test.ts — ours 槽位 WGSL 导出器（驻留 core 测试目录运行；正本在
// opencat-shaders/tools/shader-diff/export/）。走库自带 GPU-free 通路（mock root 让
// compute hook 也运行）：composeNodeTree resolve 全部 pass 的 WGSL + uniform store
// handles 读出「已 transform」的初值，落盘 wgsl/<Component>/（shader-N.wgsl +
// passes.json + values.json）。Rust 侧 ours 执行器按 naga 偏移写 uniform。
import {it} from 'vitest'
import {writeFileSync, mkdirSync, readFileSync} from 'node:fs'
import tgpu from 'typegpu'
import {composeNodeTree} from '@coreroot/gpu/composer'
import {createUniformStore, type UniformStore} from '@coreroot/gpu/uniformStore'
import {SystemUniforms} from '@coreroot/gpu/kit/coords'
import {createGpuUniformsMap} from '@coreroot/gpu/uniformBridge'
import {d} from '@coreroot/gpu/kit'
import type {GpuShaderDefinition, RegistryNode, RegistryView, NodeMetadata, GpuFragmentParams, Expr} from '@coreroot/gpu/contract'
import {getAllShaders} from '@coreroot/registry'
import {setShadersDebug} from '@coreroot/gpu/support'
setShadersDebug(true)

const catalog = JSON.parse(readFileSync('/home/solaren/Projects/opencat-shaders/tools/shader-diff/catalog.json', 'utf8'))
const OUT = '/home/solaren/Projects/opencat-shaders/tools/shader-diff/wgsl'

function mockRoot() {
    const buffer = {
        patch: () => {}, write: () => {}, destroy: () => {},
        $usage() { return buffer },
    }
    const texture = {$usage() { return texture }, destroy: () => {}, write: () => {}, props: {format: 'rgba16float'}}
    const guarded = {with() { return guarded }, dispatchThreads: () => {}}
    return {
        createBuffer: () => buffer,
        createBindGroup: () => ({}),
        createTexture: () => texture,
        createSampler: () => ({resourceType: 'sampler'}),
        createUniform: () => ({buffer: {}, write: () => {}, patch: () => {}}),
        createGuardedComputePipeline: () => guarded,
        device: {},
    } as never
}

const RootContainer: GpuShaderDefinition = {
    name: 'Root',
    props: {} as never,
    fragment: ({childNode}: GpuFragmentParams): Expr => childNode ?? (undefined as never),
}

const SYS_INITIAL: Record<string, number | number[]> = {
    time: 0,
    viewportSize: [512, 512],
    logicalViewportSize: [512, 512],
    aspect: 1,
    pointer: [0.5, 0.5],
    pointerActive: 0,
}

function buildRegistry(specs: {id: string; def: GpuShaderDefinition; parentId: string | null; props: Record<string, unknown>}[]): {registry: RegistryView; store: UniformStore; handlesById: Record<string, Record<string, any>>} {
    const root = mockRoot()
    const store = createUniformStore(root, {systemSchema: SystemUniforms as never})
    const handlesById: Record<string, Record<string, any>> = {}
    for (const s of specs) {
        const map = createGpuUniformsMap(s.def as never, s.props, s.id)
        const propFields = Object.entries(map).map(([name, u]: any) => ({name, initial: u.value, transform: u.transform, cpu: u.cpu, schema: u.schema}))
        const synthetic = [{name: '_opacity', schema: d.f32, initial: 1}]
        if (s.def.animatedTime) synthetic.push({name: '_animTime', schema: d.f32, initial: 0})
        if (s.def.extraFields) {
            for (const [name, f] of Object.entries(s.def.extraFields)) synthetic.push({name, schema: f.schema, initial: f.initial})
        }
        handlesById[s.id] = store.defineNode(s.id, [...propFields, ...synthetic]) as never
    }
    const sysHandles = store.defineSystem()
    store.finalize()

    const nodes = new Map<string, RegistryNode>()
    const childrenByParent = new Map<string, RegistryNode[]>()
    for (const s of specs) {
        nodes.set(s.id, {
            id: s.id,
            componentName: s.def.name,
            parentId: s.parentId,
            definition: s.def,
            metadata: {blendMode: 'normal', opacity: undefined, renderOrder: 0} as NodeMetadata,
            handles: handlesById[s.id],
        } as RegistryNode)
    }
    for (const s of specs) {
        if (s.parentId) {
            const arr = childrenByParent.get(s.parentId) ?? []
            arr.push(nodes.get(s.id)!)
            childrenByParent.set(s.parentId, arr)
        }
    }
    const rootNode = specs.find((x) => x.parentId === null)!
    const registry: RegistryView = {
        rootId: rootNode.id,
        getNode: (id) => nodes.get(id),
        getChildren: (parentId) => childrenByParent.get(parentId) ?? [],
        resolveCustomId: () => null,
        store,
    } as never
    return {registry, store, handlesById: {...handlesById, sys: sysHandles as never}}
}

// handle mirror → JSON 值（最终形态，Rust 直接按 naga offset 打包）。
function handleValue(h: any): unknown {
    if (!h || h.cpu) return undefined
    if (h._mirror !== undefined) {
        const m = h._mirror as number[]
        return m.length === 1 ? m[0] : m
    }
    if (h.array !== undefined) return h.array
    if (h._elementTuple !== undefined && h.componentsPerElement !== undefined) {
        const out: number[] = []
        const elems = (h._mirror?.length ?? 0) / h.componentsPerElement
        for (let i = 0; i < elems; i++) out.push(...(h._elementTuple(i) as number[]))
        return out
    }
    return undefined
}

it('exports every catalog component', () => {
    const all = new Map<string, GpuShaderDefinition>()
    for (const s of getAllShaders()) all.set(s.name, s.definition as GpuShaderDefinition)
    mkdirSync(OUT, {recursive: true})
    const summary: Record<string, string> = {}
    let ok = 0, fail = 0
    for (const comp of catalog.components) {
        try {
            const specs: {id: string; def: GpuShaderDefinition; parentId: string | null; props: Record<string, unknown>}[] = [
                {id: 'root', def: RootContainer, parentId: null, props: {}},
            ]
            const walk = (node: any, parentId: string | null, idBase: string): void => {
                const d2 = all.get(node.type)
                if (!d2) throw new Error(`component not in registry: ${node.type}`)
                const props: Record<string, unknown> = {}
                for (const [name, cfg] of Object.entries(d2.props)) props[name] = (cfg as {default: unknown}).default
                Object.assign(props, node.props ?? {})
                specs.push({id: idBase, def: d2, parentId, props})
                for (const [i, ch] of (node.children ?? []).entries()) walk(ch, idBase, `${idBase}c${i}`)
            }
            walk(comp.preset, 'root', 'n0')
            const {registry, handlesById} = buildRegistry(specs)
            // onBeforeRender 驱动协议（真 renderer 的帧序 = updateAnimatedTime → onBeforeRender
            // → flush → render，gpu/index.ts:2399）：采样帧上把回调经 setExtraField 写入的值
            // 落盘为 extraFrameValues；_animTime 由帧循环推进后经 resolveDriverCpuValue 供回调读。
            const extraSnapshots: Record<number, Record<string, number | number[]>> = {}
            const pendingExtras: Record<string, number | number[]> = {}
            const animState = new Map<string, number>()
            const advanceAnim = (frame: number): void => {
                for (const s of specs) {
                    if (!s.def.animatedTime) continue
                    const key = s.id
                    const speedProp = s.def.animatedTime.speed
                    const speed = Number(handlesById[s.id]?.[speedProp]?.value) || 0
                    const v = (animState.get(key) ?? 0) + (1 / 30) * speed
                    animState.set(key, v)
                    handlesById[s.id]!['_animTime'].value = v
                }
            }
            const ir = composeNodeTree(registry, {
                premultiplyAlpha: true,
                flipY: false, // 真 renderer（gpu/index.ts composeOptions）固定 false；composer 默认 true 会多一次翻转
                dimensions: {width: 512, height: 512},
                gpu: {device: {}, root: mockRoot()} as never,
                writeExtraField: (nodeId: string, name: string, value: number | number[]) => {
                    pendingExtras[`${nodeId}\u0000${name}`] = value
                },
                resolveDriverCpuValue: (nodeId: string, prop: string): unknown => {
                    if (prop === '_animTime' || prop.startsWith('_animTime_')) {
                        return handlesById[nodeId]?.[prop]?.value
                    }
                    return undefined
                },
            } as never)
            // 帧模拟：真 renderer 顺序 updateAnimatedTime → onBeforeRender → flush → render。
            // setExtraField 是持久写入（dirty-keyed 组件只首帧写一次，值一直有效到下次覆盖，
            // 真 renderer 直接 patch live handle），所以 pendingExtras 不逐帧清空 —— 它就是
            // 当前帧的 extraField 全量状态。
            const onBefore = (ir as {onBeforeRender: ((p: unknown) => void)[]}).onBeforeRender
            for (let frame = 1; frame <= 90; frame++) {
                advanceAnim(frame)
                for (const cb of onBefore) {
                    try {
                        cb({deltaTime: 1 / 30})
                    } catch (e) {
                        console.log(`[export-cb-error] ${comp.name} frame ${frame}: ${(e as Error).message.slice(0, 120)}`)
                    }
                }
                if (frame === 10 || frame === 45 || frame === 90) {
                    extraSnapshots[frame] = {...pendingExtras}
                }
            }
            const passes: {key: string; reads: string[]; wgsl: string}[] = []
            for (const rp of ir.rttPasses) {
                passes.push({key: rp.textureKey, reads: rp.fragment.reads, wgsl: tgpu.resolve([rp.fragment.entry], {names: 'strict'})})
            }
            passes.push({key: 'final', reads: ir.finalPass.reads, wgsl: tgpu.resolve([ir.finalPass.entry], {names: 'strict'})})
            const dir = `${OUT}/${comp.name}`
            mkdirSync(dir, {recursive: true})
            for (const [i, ps] of passes.entries()) writeFileSync(`${dir}/shader-${i}.wgsl`, ps.wgsl)
            // 字段值：node struct key（n_<id>）→ 字段名 → 已 transform 的最终值。
            const values: Record<string, Record<string, unknown>> = {}
            for (const [id, hs] of Object.entries(handlesById)) {
                const key = id === 'sys' ? '_sys' : `n_${id}`
                values[key] = {}
                for (const [name, h] of Object.entries(hs)) {
                    const v = handleValue(h)
                    if (v !== undefined) values[key][name] = v
                }
            }
            writeFileSync(`${dir}/values.json`, JSON.stringify(values, null, 1))
            writeFileSync(`${dir}/extra.json`, JSON.stringify(extraSnapshots, null, 1))
            writeFileSync(`${dir}/passes.json`, JSON.stringify({
                rtt: passes.slice(0, -1).map((ps, i) => ({index: i, key: ps.key, reads: ps.reads})),
                finalReads: ir.finalPass.reads,
                rttFormat: 'rgba16float',
                computeSteps: ir.computeSteps.length,
                computeNodeIds: ir.computeSteps.map((cs) => cs.nodeId),
                externalReads: ir.finalPass.externalReads,
            }, null, 2))
            writeFileSync(`${dir}/layout.json`, JSON.stringify({name: comp.name, role: comp.role, spec: comp.preset, sysInitial: SYS_INITIAL}, null, 2))
            summary[comp.name] = 'ok'
            ok++
        } catch (e) {
            summary[comp.name] = `FAIL: ${(e as Error).message.slice(0, 200)}`
            fail++
        }
    }
    writeFileSync(`${OUT}/export-summary.json`, JSON.stringify({ok, fail, summary}, null, 2))
    console.log(`export ok=${ok} fail=${fail}`)
})
