use raytracing::renderer::RaytracingRenderer;

#[async_std::main]
async fn main() {
    let dimension = 1024;

    let raw_bytes = RaytracingRenderer::new()
        .await
        .render_as_rgba8unorm_slice(dimension, dimension)
        .await;

    image::save_buffer("out.png", &raw_bytes, dimension, dimension, image::ColorType::Rgba8).unwrap();
}
