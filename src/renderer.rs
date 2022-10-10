use wgpu::{
    include_wgsl, Adapter, Backends, BindGroupDescriptor, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, BufferBindingType, BufferDescriptor, BufferUsages,
    CommandEncoderDescriptor, ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor,
    Device, DeviceDescriptor, Instance, Maintain, PipelineLayoutDescriptor, Queue,
    RequestAdapterOptions, ShaderStages,
};
use zerocopy::AsBytes;

#[derive(AsBytes)]
#[repr(C)]
struct RayRaw {
    origin: [f32; 3],
    direction: [f32; 3],
}

pub trait Render {
    fn render(&self);
    fn render_to_texture(&self, texture: &wgpu::Texture);
}

pub struct RaytracingRenderer {
    instance: Instance,
    adapter: Adapter,
    device: Device,
    queue: Queue,

    ray_gen_pipeline: ComputePipeline,
    ray_chit_pipeline: ComputePipeline,
    ray_ahit_pipeline: ComputePipeline,
    ray_miss_pipeline: ComputePipeline,
    ray_intersect_pipeline: ComputePipeline,
}

impl RaytracingRenderer {
    pub async fn new() -> Self {
        let instance = Instance::new(Backends::PRIMARY);

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("No suitable adapter found");

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("Main device"),
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let ray_gen_shader = device.create_shader_module(include_wgsl!("shaders/ray_gen.wgsl"));
        let ray_chit_shader = device.create_shader_module(include_wgsl!("shaders/ray_chit.wgsl"));
        let ray_ahit_shader = device.create_shader_module(include_wgsl!("shaders/ray_ahit.wgsl"));
        let ray_miss_shader = device.create_shader_module(include_wgsl!("shaders/ray_miss.wgsl"));
        let ray_intersect_shader =
            device.create_shader_module(include_wgsl!("shaders/ray_intersect.wgsl"));

        let ray_gen_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Ray generation pipeline"),
            layout: None,
            module: &ray_gen_shader,
            entry_point: "main",
        });
        let ray_chit_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Ray closest hit pipeline"),
            layout: None,
            module: &ray_chit_shader,
            entry_point: "main",
        });
        let ray_ahit_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Ray any hit pipeline"),
            layout: None,
            module: &ray_ahit_shader,
            entry_point: "main",
        });
        let ray_miss_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Ray miss pipeline"),
            layout: None,
            module: &ray_miss_shader,
            entry_point: "main",
        });
        let ray_intersect_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Ray intersect pipeline"),
            layout: None,
            module: &ray_intersect_shader,
            entry_point: "main",
        });

        Self {
            instance,
            adapter,
            device,
            queue,
            ray_gen_pipeline,
            ray_chit_pipeline,
            ray_ahit_pipeline,
            ray_miss_pipeline,
            ray_intersect_pipeline,
        }
    }

    pub async fn porcodio(&self, width: u64, height: u64) -> Vec<u8> {
        let staging_buf = self.device.create_buffer(&BufferDescriptor {
            label: Some("Ray buffer"),
            size: width * height * std::mem::size_of::<[f32; 4]>() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Ray buffer"),
            size: width * height * std::mem::size_of::<[f32; 4]>() as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let ray_gen_shader = self
            .device
            .create_shader_module(include_wgsl!("shaders/ray_gen.wgsl"));

        let bg_lay = self
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Ray generation bind group layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let bg = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Ray generation bind group"),
            layout: &bg_lay,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: staging_buf.as_entire_binding(),
            }],
        });

        let pip_lay = self
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Ray generation pipeline layout"),
                bind_group_layouts: &[&bg_lay],
                push_constant_ranges: &[],
            });

        let ray_gen_pipeline = self
            .device
            .create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Ray generation pipeline"),
                layout: Some(&pip_lay),
                module: &ray_gen_shader,
                entry_point: "main",
            });

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Ray generation command encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Ray generation compute pass"),
            });

            pass.set_bind_group(0, &bg, &[]);
            pass.set_pipeline(&ray_gen_pipeline);
            pass.dispatch_workgroups(256, 256, 64);
        }

        encoder.copy_buffer_to_buffer(
            &staging_buf,
            0,
            &buffer,
            0,
            width * height * std::mem::size_of::<[f32; 4]>() as u64,
        );

        self.queue.submit(Some(encoder.finish()));

        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        self.device.poll(Maintain::Wait);

        if let Some(Ok(())) = receiver.receive().await {
            let data = buffer.slice(..).get_mapped_range();
            let vec = data.as_bytes().to_vec();
            drop(data);

            buffer.unmap();

            vec
        } else {
            panic!("Could not map buffer");
        }
    }
}
