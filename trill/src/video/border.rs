//! GPU presentation of the custom emulator-border **video** via a custom
//! iced `wgpu` shader primitive.
//!
//! ## Why this exists
//!
//! The border video decoder (`crate::session`) produces a fresh RGBA frame
//! every video tick. Handing each one to iced's `image` widget — even via a
//! cache — flickers: `image::Handle::from_rgba` mints a new `Id::unique()`
//! per frame, and iced's wgpu image cache allocates a new atlas region,
//! uploads, and frees the previous one each frame, racing the present. This
//! is the same problem the live framebuffer hit (see
//! [`crate::video::framebuffer`]); the fix is identical: own ONE persistent
//! texture and `queue.write_texture` the new pixels into it in place, keyed
//! by a monotonic `revision` so a redraw without a new frame skips the
//! upload.
//!
//! The texture is drawn with **cover** fit (preserve aspect ratio, crop the
//! overflow) to fill the emulator backdrop, matching the old
//! `ContentFit::Cover` the image-based border used.

use std::sync::Arc;

use iced::advanced::mouse;
use iced::widget::shader::{self, Viewport};
use iced::Rectangle;

const BYTES_PER_PIXEL: u32 = 4; // RGBA8

/// One decoded border-video frame, ready to present. Cheap to clone — the
/// pixels live behind an `Arc`. `revision` is globally monotonic (assigned
/// by the decoder) so the pipeline can tell "same frame again" (skip upload)
/// from "new frame" (upload).
#[derive(Debug, Clone)]
pub struct Frame {
    pub pixels: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub revision: u64,
}

/// The iced [`shader::Program`] stored in the widget tree.
#[derive(Debug)]
pub struct Program {
    frame: Frame,
}

impl Program {
    pub fn new(frame: Frame) -> Self {
        Self { frame }
    }
}

impl<Message> shader::Program<Message> for Program {
    type State = ();
    type Primitive = Primitive;

    fn draw(&self, _state: &(), _cursor: mouse::Cursor, _bounds: Rectangle) -> Primitive {
        Primitive {
            frame: self.frame.clone(),
        }
    }
}

/// The per-frame primitive. Carries the frame into `prepare`/`draw`; the
/// persistent GPU resources live in [`Pipeline`].
#[derive(Debug)]
pub struct Primitive {
    frame: Frame,
}

impl shader::Primitive for Primitive {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
        pipeline.upload(device, queue, &self.frame);
        pipeline.write_cover(queue, bounds, &self.frame);
    }

    fn draw(&self, pipeline: &Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        pipeline.draw(render_pass);
        // We drew into the existing pass; tell iced not to call `render`.
        true
    }
}

/// The current video texture + its bind group, sized to the frame.
#[derive(Debug)]
struct FrameTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    revision: Option<u64>,
}

/// Persistent wgpu state: the render pipeline, the cover-fit uniform, the
/// sampler, and a lazily (re)created texture tracking the current video size.
#[derive(Debug)]
pub struct Pipeline {
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// `vec4<f32>` { scale_u, scale_v, _, _ } — the cover-fit UV scale.
    cover: wgpu::Buffer,
    texture: Option<FrameTexture>,
}

impl shader::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("border video bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("border video pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Bake the target gamma into a const so the shader can const-fold the
        // sRGB branch (mirrors the framebuffer effect's `SRGB_TARGET`).
        let srgb_const = if format.is_srgb() {
            "const SRGB_TARGET: bool = true;\n"
        } else {
            "const SRGB_TARGET: bool = false;\n"
        };
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("border video shader"),
            source: wgpu::ShaderSource::Wgsl(format!("{srgb_const}{SHADER}").into()),
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("border video pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("border video sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let cover = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("border video cover uniform"),
            size: 16, // vec4<f32>
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            bind_group_layout,
            sampler,
            cover,
            texture: None,
        }
    }
}

impl Pipeline {
    /// (Re)create the texture if the video size changed, then upload pixels if
    /// the resident revision differs from `frame`'s.
    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &Frame) {
        let needs_new = match &self.texture {
            Some(t) => t.width != frame.width || t.height != frame.height,
            None => true,
        };
        if needs_new {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("border video texture"),
                size: wgpu::Extent3d {
                    width: frame.width,
                    height: frame.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // Raw sRGB-encoded bytes; the shader converts to linear for an
                // sRGB target (see SHADER). A non-srgb `Unorm` view keeps the
                // stored bytes verbatim so both target gammas round-trip.
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("border video bind group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.cover.as_entire_binding(),
                    },
                ],
            });
            self.texture = Some(FrameTexture {
                texture,
                bind_group,
                width: frame.width,
                height: frame.height,
                revision: None,
            });
        }

        let tex = self.texture.as_mut().expect("texture just ensured");
        if tex.revision == Some(frame.revision) {
            return; // same frame already resident
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(frame.width * BYTES_PER_PIXEL),
                rows_per_image: Some(frame.height),
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );
        tex.revision = Some(frame.revision);
    }

    /// Compute and upload the cover-fit UV scale for the current widget
    /// bounds vs the video aspect ratio.
    fn write_cover(&self, queue: &wgpu::Queue, bounds: &Rectangle, frame: &Frame) {
        let (mut su, mut sv) = (1.0f32, 1.0f32);
        if bounds.width > 0.0 && bounds.height > 0.0 && frame.width > 0 && frame.height > 0 {
            let widget_aspect = bounds.width / bounds.height;
            let video_aspect = frame.width as f32 / frame.height as f32;
            if widget_aspect > video_aspect {
                // Widget wider than the video: fill width, crop top/bottom.
                sv = video_aspect / widget_aspect;
            } else {
                // Widget taller/narrower: fill height, crop left/right.
                su = widget_aspect / video_aspect;
            }
        }
        queue.write_buffer(&self.cover, 0, bytemuck::cast_slice(&[su, sv, 0.0f32, 0.0f32]));
    }

    fn draw(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        let Some(tex) = self.texture.as_ref() else {
            return;
        };
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &tex.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

/// Fullscreen-triangle vertex shader + cover-fit sampling fragment. The
/// `SRGB_TARGET` const is prepended by [`Pipeline::new`].
const SHADER: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    var verts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    let xy = verts[i];
    var out: VsOut;
    out.pos = vec4<f32>(xy, 0.0, 1.0);
    // Map clip space to [0,1] UV with the texture's top-left origin.
    out.uv = vec2<f32>((xy.x + 1.0) * 0.5, 1.0 - (xy.y + 1.0) * 0.5);
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> cover: vec4<f32>;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let cutoff = c <= vec3<f32>(0.04045);
    let low = c / 12.92;
    let high = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, cutoff);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Cover fit: scale the sampled region about its centre.
    let uv = (in.uv - vec2<f32>(0.5)) * cover.xy + vec2<f32>(0.5);
    let c = textureSampleLevel(tex, samp, uv, 0.0);
    if (SRGB_TARGET) {
        // Target re-encodes linear->sRGB on write; hand it linear so the
        // displayed pixel matches the source sRGB bytes.
        return vec4<f32>(srgb_to_linear(c.rgb), c.a);
    }
    // Linear target presents the value as-is; the stored sRGB bytes are
    // already display-ready.
    return c;
}
"#;
