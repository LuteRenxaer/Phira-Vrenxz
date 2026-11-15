use macroquad::prelude::*;
use miniquad::{BlendFactor, BlendState, BlendValue, Equation};

// 与 life_bar.rs 相同的顶点 shader（简化：去掉 _OpticalFOV 相关，保留 color0 透传）
const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
uniform mat4 Model;
uniform mat4 Projection;
varying vec2 vs_TEXCOORD0;
varying vec4 vs_COLOR0;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    vs_TEXCOORD0 = texcoord;
    vs_COLOR0 = color0;
}
"#;

// 模拟 life_bar FRAGMENT 末尾的 alpha 链：RGB 恒 1、w=max(rgb)=1，然后 *vs_COLOR0.w
const FRAGMENT: &str = r#"#version 100
precision highp float;
varying vec2 vs_TEXCOORD0;
varying vec4 vs_COLOR0;
void main() {
    vec4 u_xlat0 = vec4(1.0, 0.0, 0.0, 1.0);
    u_xlat0.w = u_xlat0.w * vs_COLOR0.w;
    gl_FragColor = u_xlat0;
}
"#;

fn main() {
    let conf = Conf {
        window_title: "probe_screenshot".to_owned(),
        window_width: 800,
        window_height: 400,
        high_dpi: false,
        ..Default::default()
    };
    macroquad::Window::from_config(conf, async {
        let params = MaterialParams {
            pipeline_params: PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
            ..Default::default()
        };
        let mat = load_material(VERTEX, FRAGMENT, params).expect("material load failed");

        for frame in 0..3600 {
            clear_background(Color::new(0.2, 0.2, 0.2, 1.0));

            // 左上 (0,0,400,200)：绿底 + 自定义材质红 alpha=1.0*0.6 叠加
            draw_rectangle(0.0, 0.0, 400.0, 200.0, Color::new(0.0, 1.0, 0.0, 1.0));
            gl_use_material(mat);
            draw_rectangle(0.0, 0.0, 400.0, 200.0, Color::new(1.0, 1.0, 1.0, 0.6));
            gl_use_default_material();

            // 右上 (400,0,400,200)：自定义材质红 alpha0.6 叠灰背景
            gl_use_material(mat);
            draw_rectangle(400.0, 0.0, 400.0, 200.0, Color::new(1.0, 1.0, 1.0, 0.6));
            gl_use_default_material();

            // 左下 (0,200,400,200)：默认材质红 alpha0.6 叠灰背景（对照组）
            draw_rectangle(0.0, 200.0, 400.0, 200.0, Color::new(1.0, 0.0, 0.0, 0.6));

            // 右下 (400,200,400,200)：自定义材质红 alpha=1（shader w=max=1, vs_COLOR0.w=1）叠灰背景
            gl_use_material(mat);
            draw_rectangle(400.0, 200.0, 400.0, 200.0, Color::new(1.0, 1.0, 1.0, 1.0));
            gl_use_default_material();

            next_frame().await;
            if frame == 90 {
                std::fs::write("F:\\phirLie\\probe_lifebar\\marker.txt", "frame90").ok();
                println!("PROBE90 frame reached, staying alive for external screenshot");
            }
        }
        std::fs::write("F:\\phirLie\\probe_lifebar\\done.txt", "done").ok();
        std::process::exit(0);
    });
}
