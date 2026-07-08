use glam::{Mat4, Vec2, Vec3};
use winit::{
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Orbit,
    Fly,
}

#[derive(Debug, Clone)]
pub struct Camera {
    pub mode: CameraMode,
    pub target: Vec3,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub radius: f32,
    pub fly_speed: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            mode: CameraMode::Orbit,
            target: Vec3::ZERO,
            position: Vec3::new(0.0, 42.0, 145.0),
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: -0.24,
            radius: 155.0,
            fly_speed: 75.0,
        }
    }
}

impl Camera {
    pub fn eye(&self) -> Vec3 {
        match self.mode {
            CameraMode::Orbit => self.target - self.forward() * self.radius,
            CameraMode::Fly => self.position,
        }
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize_or_zero()
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize_or_zero()
    }

    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        let eye = self.eye();
        let center = match self.mode {
            CameraMode::Orbit => self.target,
            CameraMode::Fly => eye + self.forward(),
        };
        let view = Mat4::look_at_rh(eye, center, Vec3::Y);
        let projection = Mat4::perspective_rh(48.0_f32.to_radians(), aspect.max(0.001), 0.1, 1_500.0);
        projection * view
    }
}

#[derive(Debug, Default)]
pub struct CameraController {
    rotating: bool,
    last_cursor: Option<Vec2>,
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    fast: bool,
}

impl CameraController {
    pub fn process_event(&mut self, camera: &mut Camera, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseInput { state, button, .. } if *button == MouseButton::Left => {
                self.rotating = *state == ElementState::Pressed;
                if !self.rotating {
                    self.last_cursor = None;
                }
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                let cursor = Vec2::new(position.x as f32, position.y as f32);
                if self.rotating {
                    if let Some(previous) = self.last_cursor {
                        let delta = cursor - previous;
                        camera.yaw -= delta.x * 0.006;
                        camera.pitch = (camera.pitch - delta.y * 0.005).clamp(-1.35, 1.35);
                    }
                    self.last_cursor = Some(cursor);
                    true
                } else {
                    self.last_cursor = Some(cursor);
                    false
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y * 8.0,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 * 0.12,
                };
                camera.radius = (camera.radius - scroll).clamp(24.0, 480.0);
                true
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Tab) if pressed => {
                        camera.mode = match camera.mode {
                            CameraMode::Orbit => {
                                camera.position = camera.eye();
                                CameraMode::Fly
                            }
                            CameraMode::Fly => {
                                camera.target = camera.position + camera.forward() * camera.radius;
                                CameraMode::Orbit
                            }
                        };
                        return true;
                    }
                    PhysicalKey::Code(KeyCode::KeyW) => self.forward = pressed,
                    PhysicalKey::Code(KeyCode::KeyS) => self.backward = pressed,
                    PhysicalKey::Code(KeyCode::KeyA) => self.left = pressed,
                    PhysicalKey::Code(KeyCode::KeyD) => self.right = pressed,
                    PhysicalKey::Code(KeyCode::KeyE) => self.up = pressed,
                    PhysicalKey::Code(KeyCode::KeyQ) => self.down = pressed,
                    PhysicalKey::Code(KeyCode::ShiftLeft) | PhysicalKey::Code(KeyCode::ShiftRight) => {
                        self.fast = pressed
                    }
                    _ => return false,
                }
                true
            }
            _ => false,
        }
    }

    pub fn update_camera(&self, camera: &mut Camera, dt: f32) {
        if camera.mode != CameraMode::Fly {
            return;
        }

        let mut movement = Vec3::ZERO;
        if self.forward {
            movement += camera.forward();
        }
        if self.backward {
            movement -= camera.forward();
        }
        if self.right {
            movement += camera.right();
        }
        if self.left {
            movement -= camera.right();
        }
        if self.up {
            movement += Vec3::Y;
        }
        if self.down {
            movement -= Vec3::Y;
        }

        if movement.length_squared() > 0.0 {
            let speed = camera.fly_speed * if self.fast { 2.6 } else { 1.0 };
            camera.position += movement.normalize() * speed * dt;
        }
    }
}
