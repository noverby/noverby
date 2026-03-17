use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlImageElement, WebGlRenderingContext as GL, WebGlTexture};

pub struct TextureManager {
    gl: GL,
    pub textures: Rc<RefCell<HashMap<String, WebGlTexture>>>,
}

impl TextureManager {
    pub fn new(gl: GL) -> Self {
        Self {
            gl,
            textures: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn load_icon(&self, icon: &str) {
        let gl = self.gl.clone();
        let textures = Rc::clone(&self.textures);
        let icon_name = icon.to_string();
        let src = format!("icons/{icon}");

        // Create a placeholder texture immediately
        if let Some(tex) = gl.create_texture() {
            gl.bind_texture(GL::TEXTURE_2D, Some(&tex));
            // 1x1 transparent pixel as placeholder
            let pixel: [u8; 4] = [0, 0, 0, 0];
            let _ = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                GL::TEXTURE_2D,
                0,
                GL::RGBA as i32,
                1,
                1,
                0,
                GL::RGBA,
                GL::UNSIGNED_BYTE,
                Some(&pixel),
            );
            textures.borrow_mut().insert(icon_name.clone(), tex);
        }

        let img = HtmlImageElement::new().unwrap();
        img.set_cross_origin(Some("anonymous"));

        let gl_clone = gl.clone();
        let textures_clone = Rc::clone(&textures);
        let icon_clone = icon_name.clone();
        let img_clone = img.clone();

        let onload = Closure::wrap(Box::new(move || {
            if let Some(tex) = gl_clone.create_texture() {
                gl_clone.bind_texture(GL::TEXTURE_2D, Some(&tex));
                let _ = gl_clone.tex_image_2d_with_u32_and_u32_and_image(
                    GL::TEXTURE_2D,
                    0,
                    GL::RGBA as i32,
                    GL::RGBA,
                    GL::UNSIGNED_BYTE,
                    &img_clone,
                );
                gl_clone.tex_parameteri(
                    GL::TEXTURE_2D,
                    GL::TEXTURE_WRAP_S,
                    GL::CLAMP_TO_EDGE as i32,
                );
                gl_clone.tex_parameteri(
                    GL::TEXTURE_2D,
                    GL::TEXTURE_WRAP_T,
                    GL::CLAMP_TO_EDGE as i32,
                );
                gl_clone.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::LINEAR as i32);
                gl_clone.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::LINEAR as i32);
                textures_clone.borrow_mut().insert(icon_clone.clone(), tex);
            }
        }) as Box<dyn FnMut()>);

        img.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget(); // Leak the closure — it only fires once

        img.set_src(&src);
    }
}
