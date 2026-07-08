use crate::{
    camera::{Camera, CameraController},
    renderer::{AgentRenderer, TrailRenderer},
    simulation::{FlockSimulation, SimulationSettings},
    ui::{self, UiState},
};
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use std::{sync::Arc, time::Instant};
use wgpu::util::DeviceExt;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    eye_position: [f32; 4],
    params: [f32; 4],
}

impl CameraUniform {
    pub fn new(camera: &Camera, aspect: f32, elapsed: f32, bounds: f32) -> Self {
        Self {
            view_proj: camera.view_projection(aspect).to_cols_array_2d(),
            eye_position: camera.eye().extend(1.0).to_array(),
            params: [elapsed, bounds, 0.0, 0.0],
        }
    }
}

pub struct GpuApp {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    depth: DepthTexture,
    camera: Camera,
    camera_controller: CameraController,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    agent_renderer: AgentRenderer,
    trail_renderer: TrailRenderer,
    simulation: FlockSimulation,
    ui_state: UiState,
    egui_context: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    last_frame: Instant,
    started_at: Instant,
}

impl GpuApp {
    pub async fn new(window: Window) -> Result<Self> {
        let window = Arc::new(window);
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::default(),
            dx12_shader_compiler: Default::default(),
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
        });
        let surface = instance
            .create_surface(window.clone())
            .context("failed to create wgpu surface")?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("no compatible GPU adapter found")?;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("flock lab device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .context("failed to create logical GPU device")?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .unwrap_or(capabilities.formats[0]);
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::Mailbox)
            .unwrap_or(wgpu::PresentMode::Fifo);
        let alpha_mode = capabilities.alpha_modes[0];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let depth = DepthTexture::create(&device, &config);

        let camera = Camera::default();
        let simulation = FlockSimulation::new(SimulationSettings::default());
        let camera_uniform = CameraUniform::new(
            &camera,
            config.width as f32 / config.height as f32,
            0.0,
            simulation.settings.bounds,
        );
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera uniform buffer"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let mut agent_renderer =
            AgentRenderer::new(&device, config.format, &camera_bind_group_layout, DEPTH_FORMAT);
        let mut trail_renderer =
            TrailRenderer::new(&device, config.format, &camera_bind_group_layout, DEPTH_FORMAT);
        let ui_state = UiState::default();
        agent_renderer.update_instances(&device, &queue, &simulation);
        trail_renderer.update_lines(
            &device,
            &queue,
            &simulation,
            ui_state.show_trails,
            ui_state.show_neighbor_radius,
            ui_state.show_bounds,
        );

        let egui_context = egui::Context::default();
        egui_context.set_visuals(egui::Visuals::dark());
        let egui_state = egui_winit::State::new(
            egui_context.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(&device, config.format, None, 1);

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            depth,
            camera,
            camera_controller: CameraController::default(),
            camera_buffer,
            camera_bind_group,
            agent_renderer,
            trail_renderer,
            simulation,
            ui_state,
            egui_context,
            egui_state,
            egui_renderer,
            last_frame: Instant::now(),
            started_at: Instant::now(),
        })
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth = DepthTexture::create(&self.device, &self.config);
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        let egui_response = self.egui_state.on_window_event(&self.window, event);
        if egui_response.consumed {
            return true;
        }

        if self.camera_controller.process_event(&mut self.camera, event) {
            return true;
        }

        if let WindowEvent::KeyboardInput { event, .. } = event {
            if event.state == ElementState::Pressed {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Space) => {
                        self.simulation.settings.pause = !self.simulation.settings.pause;
                        return true;
                    }
                    PhysicalKey::Code(KeyCode::KeyR) => {
                        self.simulation.reset();
                        return true;
                    }
                    PhysicalKey::Code(KeyCode::Enter) => {
                        self.simulation.randomize();
                        return true;
                    }
                    _ => {}
                }
            }
        }

        false
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.simulation.update(dt);

        let aspect = self.config.width as f32 / self.config.height.max(1) as f32;
        let camera_uniform = CameraUniform::new(
            &self.camera,
            aspect,
            self.started_at.elapsed().as_secs_f32(),
            self.simulation.settings.bounds,
        );
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));
        self.agent_renderer
            .update_instances(&self.device, &self.queue, &self.simulation);
        self.trail_renderer.update_lines(
            &self.device,
            &self.queue,
            &self.simulation,
            self.ui_state.show_trails,
            self.ui_state.show_neighbor_radius,
            self.ui_state.show_bounds,
        );
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame encoder"),
            });

        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_context.run(raw_input, |ctx| {
            let action = ui::draw(
                ctx,
                &mut self.simulation.settings,
                self.simulation.stats,
                &mut self.ui_state,
            );
            if action.reset {
                self.simulation.reset();
            }
            if action.randomize {
                self.simulation.randomize();
            }
        });
        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output.clone());

        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };
        let paint_jobs = self
            .egui_context
            .tessellate(full_output.shapes, screen_descriptor.pixels_per_point);
        let _callback_commands = self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("flock scene pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.012,
                            g: 0.016,
                            b: 0.026,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.trail_renderer.render(&mut pass, &self.camera_bind_group);
            self.agent_renderer.render(&mut pass, &self.camera_bind_group);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.egui_renderer
                .render(&mut pass, &paint_jobs, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
        Ok(())
    }
}

struct DepthTexture {
    view: wgpu::TextureView,
}

impl DepthTexture {
    fn create(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { view }
    }
}
