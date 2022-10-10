
struct Ray {
    origin: vec3<f32>,
    direction: vec3<f32>,
}

fn ray_at(ray: Ray, t: f32) -> vec3<f32> {
    return ray.origin + ray.direction * t;
}

fn hit_sphere(center: vec3<f32>, radius: f32, ray: Ray) -> f32 {
    let oc = ray.origin - center;
    let a = pow(length(ray.direction), 2.0);
    let half_b = dot(oc, ray.direction);
    let c = pow(length(oc), 2.0) - radius*radius;
    let discriminant = half_b*half_b - a*c;
    if (discriminant < 0.0) {
        return -1.0;
    } else {
        return (-half_b - sqrt(discriminant) ) / a;
    }
}

fn ray_color(ray: Ray) -> vec3<f32> {
    var t = hit_sphere(vec3<f32>(0.0, 0.0, -1.0), 0.5, ray);
    if (t > 0.0) {
        let N = normalize(ray_at(ray, t) - vec3<f32>(0.0, 0.0, -1.0));
        return 0.5 * vec3<f32>(N.x + 1.0, N.y + 1.0, N.z + 1.0);
    }
    let unit_direction = normalize(ray.direction);
    t = 0.5 * (unit_direction.y + 1.0);
    return (1.0 - t) * vec3<f32>(1.0, 1.0, 1.0) + t * vec3<f32>(0.5, 0.7, 1.0);
}

struct ComputeInput {
    @builtin(global_invocation_id) global_invocation_id: vec3<u32>,
}

@group(0) @binding(0)
var<storage, read_write> imagebuffer: array<vec4<f32>>;

@compute
@workgroup_size(1)
fn main(in: ComputeInput) {

    let aspect_ratio = 1.0;

    // Camera
    let viewport_height = 2.0;
    let viewport_width = aspect_ratio * viewport_height;
    let focal_length = 1.0;

    let origin = vec3<f32>(0.0, 0.0, 0.0);
    let horizontal = vec3<f32>(viewport_width, 0.0, 0.0);
    let vertical = vec3<f32>(0.0, viewport_height, 0.0);
    let lower_left_corner = origin - horizontal/2.0 - vertical/2.0 - vec3<f32>(0.0, 0.0, focal_length);

    let i = in.global_invocation_id.x;
    let j = in.global_invocation_id.y;

    let u = f32(i) / 256.0;
    let v = f32(j) / 256.0;

    var ray: Ray;
    ray.origin = origin;
    ray.direction = lower_left_corner + u * horizontal + v * vertical - origin;

    imagebuffer[i + j * 256u] = vec4<f32>(ray_color(ray), 1.0);
}
