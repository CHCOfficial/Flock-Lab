use anyhow::Result;
use flock_lab::gpu::GpuApp;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

fn main() -> Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Flock Lab")
        .with_inner_size(winit::dpi::PhysicalSize::new(1440, 960))
        .with_min_inner_size(winit::dpi::PhysicalSize::new(900, 600))
        .build(&event_loop)?;

    let mut app = pollster::block_on(GpuApp::new(window))?;

    event_loop.run(move |event, target| {
        target.set_control_flow(ControlFlow::Poll);
        match event {
            Event::WindowEvent { window_id, event } if window_id == app.window().id() => {
                if should_exit(&event) {
                    target.exit();
                    return;
                }

                match event {
                    WindowEvent::Resized(size) => app.resize(size),
                    WindowEvent::RedrawRequested => {
                        app.update();
                        match app.render() {
                            Ok(()) => {}
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                app.resize(app.window().inner_size());
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                            Err(error) => log::warn!("surface error: {error:?}"),
                        }
                    }
                    _ => {
                        app.input(&event);
                    }
                }
            }
            Event::AboutToWait => {
                app.window().request_redraw();
            }
            _ => {}
        }
    })?;

    Ok(())
}

fn should_exit(event: &WindowEvent) -> bool {
    match event {
        WindowEvent::CloseRequested => true,
        WindowEvent::KeyboardInput { event, .. }
            if event.state == ElementState::Pressed
                && matches!(event.physical_key, PhysicalKey::Code(KeyCode::Escape)) =>
        {
            true
        }
        _ => false,
    }
}
