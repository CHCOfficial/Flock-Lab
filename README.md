# Flock Lab

Flock Lab is a native Rust/wgpu flocking and crowd simulation demo for macOS. It renders thousands of agents as small lit 3D drones using GPU instancing, with fading motion trails, predators, obstacles, goal seeking, and debug visualisation for neighbour radii.

<img width="1280" height="750" alt="A108989C-A3FE-485D-B375-C619CACE72B3_1_206_a" src="https://github.com/user-attachments/assets/437d9131-a194-40ff-aaed-2b340e5972eb" />


## Run

```sh
cargo run --release
```

The app uses `winit` for the native window, `wgpu` for rendering, and `egui` for live controls.

## Controls

- Drag with the left mouse button to orbit or look around.
- Mouse wheel zooms the orbit camera.
- `Tab` toggles orbit/fly camera.
- Fly mode: `W/A/S/D` move, `Q/E` descend/ascend, `Shift` boosts.
- `Space` pauses, `R` resets, `Enter` randomises.
- The right panel controls agent count, speed, bounds, trail length, flocking strengths, predator count, obstacle count, and debug overlays.

## How The Flock Works

Each boid samples nearby agents through a CPU spatial hash. The core behaviours are:

- **Separation:** steer away from close neighbours.
- **Alignment:** steer toward the average velocity of neighbours.
- **Cohesion:** steer toward the local centre of mass.
- **Predator avoidance:** flee moving predator agents with distance falloff.
- **Goal seeking:** gently bias the flock toward an animated target.
- **Obstacle avoidance:** push agents out of obstacle safety radii.
- **Bounds handling:** apply a soft inward force near the simulation edge and clamp/reflection at the boundary.

The simulation path is CPU based so the behaviours are easy to inspect and unit test. Rendering is GPU driven: each agent, predator, and obstacle is one instance of a small mesh. The CPU uploads compact transform/color instance data each frame, and WGSL shaders handle projection, normal lighting, rim highlights, and fog.

## Performance Notes

The default scene starts at 3,500 agents with short trails. Increase the agent count gradually; the spatial hash keeps neighbour lookups local, but very large counts are still CPU-limited because the demo favours readable flock logic over a compute-shader implementation. The renderer is intentionally lightweight: one instanced draw for agents and one line draw for trails/debug geometry.

## Project Layout

```text
src/
  simulation/
    boids.rs
    spatial_hash.rs
  renderer/
    agent_renderer.rs
    trail_renderer.rs
  camera/
  gpu/
  ui/
  utils/

shaders/
  agents.wgsl
  trails.wgsl
```

## Tests

```sh
cargo test
```

The tests cover spatial hash indexing/querying, boid separation force direction, bounds reflection, and settings validation.
