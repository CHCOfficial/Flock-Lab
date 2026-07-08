use crate::{
    simulation::FlockSimulation,
    utils::{lerp_color, spectral_color},
};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct LineVertex {
    position: [f32; 3],
    color: [f32; 4],
}

impl LineVertex {
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
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub struct TrailRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    vertex_count: u32,
}

impl TrailRenderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        camera_layout: &wgpu::BindGroupLayout,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("trails shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/trails.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("trails pipeline layout"),
            bind_group_layouts: &[camera_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("trails pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[LineVertex::layout()],
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
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let vertex_capacity = 1;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("trail vertex buffer"),
            size: std::mem::size_of::<LineVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            vertex_capacity,
            vertex_count: 0,
        }
    }

    pub fn update_lines(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        simulation: &FlockSimulation,
        show_trails: bool,
        show_neighbor_radius: bool,
        show_bounds: bool,
    ) {
        let mut vertices = Vec::new();
        if show_trails {
            add_trails(&mut vertices, simulation);
        }
        if show_neighbor_radius {
            add_neighbor_rings(&mut vertices, simulation);
        }
        if show_bounds {
            add_bounds(&mut vertices, simulation.settings.bounds);
        }
        add_goal_marker(&mut vertices, simulation.goal);
        add_obstacle_rings(&mut vertices, simulation);

        if vertices.len() > self.vertex_capacity {
            self.vertex_capacity = vertices.len().next_power_of_two();
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("trail vertex buffer"),
                size: (self.vertex_capacity * std::mem::size_of::<LineVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        if !vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        }
        self.vertex_count = vertices.len() as u32;
    }

    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        if self.vertex_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..self.vertex_count, 0..1);
    }
}

fn add_trails(vertices: &mut Vec<LineVertex>, simulation: &FlockSimulation) {
    for (index, trail) in simulation.trails.iter().enumerate() {
        if trail.len() < 2 {
            continue;
        }
        let hue = simulation.boids.get(index).map(|b| b.hue).unwrap_or(0.0);
        for segment in 1..trail.len() {
            let t0 = (segment - 1) as f32 / trail.len().saturating_sub(1) as f32;
            let t1 = segment as f32 / trail.len().saturating_sub(1) as f32;
            let mut c0 = spectral_color(hue + t0 * 0.04, 0.8);
            let mut c1 = spectral_color(hue + t1 * 0.04, 1.0);
            c0[3] = t0.powf(1.6) * 0.24;
            c1[3] = t1.powf(1.6) * 0.38;
            vertices.push(LineVertex {
                position: trail[segment - 1].to_array(),
                color: c0,
            });
            vertices.push(LineVertex {
                position: trail[segment].to_array(),
                color: c1,
            });
        }
    }
}

fn add_neighbor_rings(vertices: &mut Vec<LineVertex>, simulation: &FlockSimulation) {
    let color = [0.25, 0.82, 1.0, 0.13];
    for boid in simulation.boids.iter().step_by((simulation.boids.len() / 18).max(1)).take(18) {
        add_ring(vertices, boid.position, simulation.settings.neighbor_radius, Vec3::X, Vec3::Z, color);
        add_ring(
            vertices,
            boid.position,
            simulation.settings.neighbor_radius,
            Vec3::X,
            Vec3::Y,
            [color[0], color[1], color[2], 0.08],
        );
    }
}

fn add_bounds(vertices: &mut Vec<LineVertex>, bounds: f32) {
    let c = [0.42, 0.58, 0.72, 0.18];
    let b = bounds;
    let corners = [
        Vec3::new(-b, -b, -b),
        Vec3::new(b, -b, -b),
        Vec3::new(b, -b, b),
        Vec3::new(-b, -b, b),
        Vec3::new(-b, b, -b),
        Vec3::new(b, b, -b),
        Vec3::new(b, b, b),
        Vec3::new(-b, b, b),
    ];
    for (a, b) in [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ] {
        line(vertices, corners[a], corners[b], c, c);
    }
}

fn add_goal_marker(vertices: &mut Vec<LineVertex>, goal: Vec3) {
    let a = [0.5, 1.0, 0.64, 0.42];
    let b = [0.5, 1.0, 0.64, 0.08];
    add_ring(vertices, goal, 5.0, Vec3::X, Vec3::Z, a);
    add_ring(vertices, goal, 7.0, Vec3::X, Vec3::Y, b);
    add_ring(vertices, goal, 7.0, Vec3::Y, Vec3::Z, b);
}

fn add_obstacle_rings(vertices: &mut Vec<LineVertex>, simulation: &FlockSimulation) {
    let color = [1.0, 0.67, 0.25, 0.3];
    for obstacle in &simulation.obstacles {
        add_ring(vertices, obstacle.position, obstacle.radius, Vec3::X, Vec3::Z, color);
        add_ring(
            vertices,
            obstacle.position,
            obstacle.radius * 1.2,
            Vec3::X,
            Vec3::Y,
            [color[0], color[1], color[2], 0.14],
        );
    }
}

fn add_ring(
    vertices: &mut Vec<LineVertex>,
    center: Vec3,
    radius: f32,
    axis_a: Vec3,
    axis_b: Vec3,
    color: [f32; 4],
) {
    let segments = 48;
    for i in 0..segments {
        let t0 = i as f32 / segments as f32 * std::f32::consts::TAU;
        let t1 = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let p0 = center + axis_a * t0.cos() * radius + axis_b * t0.sin() * radius;
        let p1 = center + axis_a * t1.cos() * radius + axis_b * t1.sin() * radius;
        let c0 = lerp_color(color, [color[0], color[1], color[2], color[3] * 0.45], i as f32 / segments as f32);
        line(vertices, p0, p1, c0, color);
    }
}

fn line(vertices: &mut Vec<LineVertex>, a: Vec3, b: Vec3, ca: [f32; 4], cb: [f32; 4]) {
    vertices.push(LineVertex {
        position: a.to_array(),
        color: ca,
    });
    vertices.push(LineVertex {
        position: b.to_array(),
        color: cb,
    });
}
