use std::num::{NonZeroU32, NonZeroU64};

use wgpu::{
    include_wgsl,
    util::{BufferInitDescriptor, DeviceExt},
    Adapter, Backends, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, BufferBindingType, BufferDescriptor, BufferUsages,
    CommandEncoderDescriptor, ComputePassDescriptor, ComputePipelineDescriptor,
    Device, DeviceDescriptor, Instance, Maintain, PipelineLayoutDescriptor,
    Queue, RequestAdapterOptions, ShaderStages, BindingResource, ImageCopyBuffer, ImageDataLayout,
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
    _instance: Instance,
    _adapter: Adapter,
    device: Device,
    queue: Queue,
}

impl RaytracingRenderer {
    pub async fn new() -> Self {
        let _instance = Instance::new(Backends::PRIMARY);

        let _adapter = _instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("No suitable adapter found");

        let (device, queue) = _adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("Main device"),
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("Failed to create device");

        Self {
            _instance,
            _adapter,
            device,
            queue,
        }
    }

    pub async fn render_as_rgba8unorm_slice(&self, width: u32, height: u32) -> Vec<u8> {
        let out_tex_extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let out_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Output texture"),
            dimension: wgpu::TextureDimension::D2,
            sample_count: 1,
            mip_level_count: 1,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::STORAGE_BINDING,
            format: wgpu::TextureFormat::Rgba8Unorm,
            size: out_tex_extent,
        });

        let out_tex_view = out_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let out_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Output buffer"),
            size: (width * height * 4) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let in_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Input buffer"),
            contents: [width, height].as_bytes(),
            usage: BufferUsages::UNIFORM,
        });

        let ray_gen_shader = self
            .device
            .create_shader_module(include_wgsl!("shaders/ray_gen.wgsl"));

        let bg_lay = self
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Ray generation bind group layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(8),
                        },
                        count: None,
                    },
                ],
            });

        let bg = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Ray generation bind group"),
            layout: &bg_lay,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&out_tex_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: in_buffer.as_entire_binding(),
                },
            ],
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
            pass.dispatch_workgroups(8192, 8192, 1);
        }

        encoder.copy_texture_to_buffer(
            out_tex.as_image_copy(),
            ImageCopyBuffer {
                buffer: &out_buffer,
                layout: ImageDataLayout {
                    bytes_per_row: NonZeroU32::new(4 * width),
                    rows_per_image: NonZeroU32::new(height),
                    offset: 0,
                },
            },
            out_tex_extent,
        );

        self.queue.submit(Some(encoder.finish()));

        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        out_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        self.device.poll(Maintain::Wait);

        if let Some(Ok(())) = receiver.receive().await {
            let data = out_buffer.slice(..).get_mapped_range();
            let vec = data.as_bytes().to_vec();
            drop(data);

            out_buffer.unmap();

            vec
        } else {
            panic!("Could not map buffer");
        }
    }
}
