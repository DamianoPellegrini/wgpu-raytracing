use raytracing::renderer::RaytracingRenderer;
use zerocopy::FromBytes;

#[async_std::main]
async fn main() {
    let dimension = 256;

    let raw_bytes = RaytracingRenderer::new()
        .await
        .porcodio(dimension, dimension)
        .await;

    let mut img: image::RgbaImage = image::ImageBuffer::new(dimension as u32, dimension as u32);

    let image_pixels = raw_bytes
        .chunks(16)
        .map(|chunk| {
            let color = <[f32; 4]>::read_from(chunk).unwrap();
            image::Rgba([
                (color[0] * 255.0) as u8,
                (color[1] * 255.0) as u8,
                (color[2] * 255.0) as u8,
                (color[3] * 255.0) as u8,
            ])
        })
        .collect::<Vec<_>>();

    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = image_pixels[(y * 256 + x) as usize];
    }

    img.save("out.png").unwrap();
}
