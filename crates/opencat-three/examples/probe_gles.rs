//! 临时探测：打印 wgpu GL 适配器与设备创建细节（Linux 离线路径排障用）。
use pollster::block_on;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::GL;
    let mut desc = desc.with_env();
    desc.flags
        .remove(wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL);
    let instance = wgpu::Instance::new(desc);
    let adapters = block_on(instance.enumerate_adapters(wgpu::Backends::GL));
    for a in &adapters {
        let info = a.get_info();
        println!(
            "adapter: {} | vendor {:x} | {:?} | backend {:?}",
            info.name, info.vendor, info.device_type, info.backend
        );
    }
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| anyhow::anyhow!("adapter: {e}"))?;
    println!("chosen: {}", adapter.get_info().name);
    match block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())) {
        Ok((_d, _q)) => println!("device OK"),
        Err(e) => println!("device FAIL: {e}"),
    }
    Ok(())
}
