struct Camera {
    view_proj: mat4x4<f32>,
    eye_position: vec4<f32>,
    params: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    var out: VertexOutput;
    out.clip_position = camera.view_proj * world;
    out.world_position = world.xyz;
    out.normal = normalize((model * vec4<f32>(input.normal, 0.0)).xyz);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.normal);
    let light = normalize(vec3<f32>(-0.32, 0.78, 0.46));
    let view_dir = normalize(camera.eye_position.xyz - input.world_position);
    let diffuse = max(dot(normal, light), 0.0);
    let rim = pow(1.0 - max(dot(normal, view_dir), 0.0), 2.2);
    let half_vec = normalize(light + view_dir);
    let spec = pow(max(dot(normal, half_vec), 0.0), 42.0);
    let pulse = 0.92 + 0.08 * sin(camera.params.x * 1.7 + input.world_position.y * 0.05);
    var rgb = input.color.rgb * (0.22 + diffuse * 0.78) * pulse;
    rgb += vec3<f32>(0.22, 0.55, 1.0) * rim * 0.28;
    rgb += vec3<f32>(1.0, 0.92, 0.76) * spec * 0.34;
    let distance = length(camera.eye_position.xyz - input.world_position);
    let fog = smoothstep(camera.params.y * 1.3, camera.params.y * 3.0, distance);
    let fog_color = vec3<f32>(0.018, 0.024, 0.038);
    return vec4<f32>(mix(rgb, fog_color, fog), input.color.a);
}
