use crate::{
    gpu::CameraUniform,
    simulation::{FlockSimulation, Obstacle},
    utils::spectral_color,
};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct AgentVertex {
    position: [f32; 3],
    normal: [f32; 3],
}

impl AgentVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct AgentInstanceRaw {
    model: [[f32; 4]; 4],
    color: [f32; 4],
}

impl AgentInstanceRaw {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            2 => Float32x4,
            3 => Float32x4,
            4 => Float32x4,
            5 => Float32x4,
            6 => Float32x4
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBUTES,
        }
    }
}

pub struct AgentRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    index_count: u32,
    instance_count: u32,
}

impl AgentRenderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        camera_layout: &wgpu::BindGroupLayout,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("agents shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/agents.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("agents pipeline layout"),
            bind_group_layouts: &[camera_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("agents pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[AgentVertex::layout(), AgentInstanceRaw::layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let vertices = dart_vertices();
        let indices = dart_indices();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("agent vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("agent index buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let instance_capacity = 1;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("agent instance buffer"),
            size: std::mem::size_of::<AgentInstanceRaw>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            instance_capacity,
            index_count: indices.len() as u32,
            instance_count: 0,
        }
    }

    pub fn update_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        simulation: &FlockSimulation,
    ) {
        let mut instances = Vec::with_capacity(
            simulation.boids.len() + simulation.predators.len() + simulation.obstacles.len(),
        );

        for boid in &simulation.boids {
            let speed_t = (boid.velocity.length() / simulation.settings.max_speed).clamp(0.0, 1.4);
            let mut color = spectral_color(boid.hue + speed_t * 0.08, 0.7 + speed_t * 0.35);
            color[3] = 0.94;
            instances.push(instance_for(boid.position, boid.velocity, 0.82 + speed_t * 0.35, color));
        }

        for predator in &simulation.predators {
            instances.push(instance_for(
                predator.position,
                predator.velocity,
                3.2,
                [1.0, 0.14, 0.08, 1.0],
            ));
        }

        for obstacle in &simulation.obstacles {
            instances.push(obstacle_instance(obstacle));
        }

        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("agent instance buffer"),
                size: (self.instance_capacity * std::mem::size_of::<AgentInstanceRaw>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        if !instances.is_empty() {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }
        self.instance_count = instances.len() as u32;
    }

    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        if self.instance_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..self.index_count, 0, 0..self.instance_count);
    }
}

fn instance_for(position: Vec3, velocity: Vec3, scale: f32, color: [f32; 4]) -> AgentInstanceRaw {
    let direction = velocity.normalize_or_zero();
    let direction = if direction.length_squared() > 0.0 {
        direction
    } else {
        Vec3::Z
    };
    let rotation = Quat::from_rotation_arc(Vec3::Z, direction);
    let model = Mat4::from_scale_rotation_translation(Vec3::splat(scale), rotation, position);
    AgentInstanceRaw {
        model: model.to_cols_array_2d(),
        color,
    }
}

fn obstacle_instance(obstacle: &Obstacle) -> AgentInstanceRaw {
    let rotation = Quat::from_rotation_y(obstacle.position.x * 0.03)
        * Quat::from_rotation_x(obstacle.position.z * 0.02);
    let model = Mat4::from_scale_rotation_translation(
        Vec3::new(obstacle.radius, obstacle.radius * 0.82, obstacle.radius),
        rotation,
        obstacle.position,
    );
    AgentInstanceRaw {
        model: model.to_cols_array_2d(),
        color: [0.92, 0.68, 0.28, 0.78],
    }
}

fn dart_vertices() -> [AgentVertex; 7] {
    [
        AgentVertex {
            position: [0.0, 0.0, 1.35],
            normal: [0.0, 0.2, 1.0],
        },
        AgentVertex {
            position: [-0.48, -0.08, -0.35],
            normal: [-0.6, 0.2, 0.3],
        },
        AgentVertex {
            position: [0.48, -0.08, -0.35],
            normal: [0.6, 0.2, 0.3],
        },
        AgentVertex {
            position: [0.0, 0.22, -0.84],
            normal: [0.0, 0.9, -0.2],
        },
        AgentVertex {
            position: [0.0, -0.32, -0.72],
            normal: [0.0, -1.0, -0.2],
        },
        AgentVertex {
            position: [-0.16, 0.03, -1.12],
            normal: [-0.4, 0.3, -0.7],
        },
        AgentVertex {
            position: [0.16, 0.03, -1.12],
            normal: [0.4, 0.3, -0.7],
        },
    ]
}

fn dart_indices() -> [u16; 24] {
    [
        0, 1, 3, 0, 3, 2, 0, 4, 1, 0, 2, 4, 1, 5, 3, 2, 3, 6, 1, 4, 5, 2, 6, 4,
    ]
}

#[allow(dead_code)]
fn _assert_camera_uniform_pod(_: CameraUniform) {}
