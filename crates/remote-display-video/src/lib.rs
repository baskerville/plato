use ciborium::{cbor, into_writer, value::Value};
use image::imageops::{grayscale_with_type, FilterType};
use image::{DynamicImage, ImageFormat, Rgb};
use std::cell::RefCell;
use std::io::Cursor;
use wasm_bindgen::prelude::*;
use web_sys::console::log_1;

// Store the previous frame for delta updates in the future
thread_local! {
    static PREVIOUS_QOI: RefCell<Option<Vec<u8>>> = RefCell::new(None);
}

#[wasm_bindgen]
pub fn png_to_display_update(
    png_data: &[u8],
    width: u32,
    height: u32,
    keyframe: bool,
) -> Result<Vec<u8>, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    log_1(&JsValue::from_str(&format!(
        "Input PNG size: {} bytes",
        png_data.len()
    )));

    // Load and resize the PNG image
    let img: DynamicImage = image::load_from_memory(png_data)
        .map_err(|e| JsValue::from_str(&format!("Failed to load image: {:?}", e)))?
        .resize_exact(width, height, FilterType::Nearest);
    let img = grayscale_with_type::<Rgb<u8>, DynamicImage>(&img);

    let mut img_writer = Cursor::new(Vec::new());
    img.write_to(&mut img_writer, image::ImageFormat::Qoi)
        .map_err(|e| JsValue::from_str(&format!("Failed to write QOI image: {:?}", e)))?;

    log_1(&JsValue::from_str(&format!(
        "QOI image size: {} bytes",
        img_writer.get_ref().len()
    )));

    // Store previous QOI image for delta updates
    let previous_qoi = PREVIOUS_QOI.with(|prev| {
        let result = prev.borrow().clone();
        *prev.borrow_mut() = Some(img_writer.get_ref().clone());
        if keyframe {
            None
        } else {
            result
        }
    });

    let (update_type, value) = if let Some(prev_img) = previous_qoi {
        let mut delta = Cursor::new(Vec::new());
        bidiff::simple_diff(
            prev_img.as_slice(),
            img_writer.get_ref().as_slice(),
            &mut delta,
        )
        .map_err(|e| JsValue::from_str(&format!("Failed to calculate delta: {:?}", e)))?;

        let delta_data = delta.into_inner();
        log_1(&JsValue::from_str(&format!(
            "Delta size: {} bytes",
            delta_data.len()
        )));

        ("patchDisplay", delta_data)
    } else {
        ("updateDisplay", img_writer.get_ref().to_vec())
    };

    let compressed = zstd::stream::encode_all(value.as_slice(), 3)
        .map_err(|e| JsValue::from_str(&format!("Failed to compress image: {:?}", e)))?;

    log_1(&JsValue::from_str(&format!(
        "Compressed size: {} bytes",
        compressed.len()
    )));

    const MAX_SIZE: usize = 400_000; // 500KB
    const FALLBACK_FORMAT: ImageFormat = ImageFormat::Jpeg;
    if compressed.len() > MAX_SIZE {
        log_1(&JsValue::from_str(&format!(
            "Falling back to {:?} due to payload size",
            FALLBACK_FORMAT
        )));
        // Fallback to WebP encoding for large updates
        let mut fallback_writer = Cursor::new(Vec::new());
        img.write_to(&mut fallback_writer, FALLBACK_FORMAT)
            .map_err(|e| {
                JsValue::from_str(&format!(
                    "Failed to write {:?} image: {:?}",
                    FALLBACK_FORMAT, e
                ))
            })?;

        // Compress the fallback data
        log_1(&JsValue::from_str(&format!(
            "{:?} size: {} bytes",
            FALLBACK_FORMAT,
            fallback_writer.get_ref().len()
        )));

        let fallback_compressed = zstd::stream::encode_all(fallback_writer.get_ref().as_slice(), 3)
            .map_err(|e| {
                JsValue::from_str(&format!(
                    "Failed to compress {:?} image: {:?}",
                    FALLBACK_FORMAT, e
                ))
            })?;

        log_1(&JsValue::from_str(&format!(
            "Compressed {:?} size: {} bytes",
            FALLBACK_FORMAT,
            fallback_compressed.len()
        )));

        // Reset the previous frame storage to prevent diffing
        PREVIOUS_QOI.with(|prev| {
            *prev.borrow_mut() = None;
        });

        // Create a special fallback update type for the fallback
        let binary_value = Value::Bytes(fallback_compressed);
        let response = cbor!({ "type" => "updateDisplay", "value" => binary_value })
            .map_err(|e| JsValue::from_str(&format!("Failed to encode response: {:?}", e)))?;
        let mut writer = Cursor::new(Vec::new());
        into_writer(&response, &mut writer)
            .map_err(|e| JsValue::from_str(&format!("Failed to write response: {:?}", e)))?;

        let final_data = writer.into_inner();
        log_1(&JsValue::from_str(&format!(
            "Final response size: {} bytes",
            final_data.len()
        )));

        return Ok(final_data);
    }

    let binary_value = Value::Bytes(compressed);
    let response = cbor!({ "type" => update_type, "value" => binary_value })
        .map_err(|e| JsValue::from_str(&format!("Failed to encode response: {:?}", e)))?;
    let mut writer = Cursor::new(Vec::new());
    into_writer(&response, &mut writer)
        .map_err(|e| JsValue::from_str(&format!("Failed to write response: {:?}", e)))?;

    let final_data = writer.into_inner();
    log_1(&JsValue::from_str(&format!(
        "Final response size: {} bytes",
        final_data.len()
    )));

    Ok(final_data)
}
