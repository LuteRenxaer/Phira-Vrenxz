#version 100
precision mediump float;

varying lowp vec2 uv;
uniform sampler2D screenTexture;
uniform vec2 screenSize;
uniform float blurSize; // %4.0% 0..20

void main() {
  vec2 pixel_size = blurSize / screenSize;
  vec4 c = vec4(0.0);
  // 9-tap gaussian blur
  c += texture2D(screenTexture, uv) * 0.227027;
  c += texture2D(screenTexture, uv + vec2(pixel_size.x, 0.0)) * 0.1945946;
  c += texture2D(screenTexture, uv - vec2(pixel_size.x, 0.0)) * 0.1945946;
  c += texture2D(screenTexture, uv + vec2(0.0, pixel_size.y)) * 0.1945946;
  c += texture2D(screenTexture, uv - vec2(0.0, pixel_size.y)) * 0.1945946;
  c += texture2D(screenTexture, uv + pixel_size) * 0.1216216;
  c += texture2D(screenTexture, uv - pixel_size) * 0.1216216;
  c += texture2D(screenTexture, uv + vec2(pixel_size.x, -pixel_size.y)) * 0.1216216;
  c += texture2D(screenTexture, uv + vec2(-pixel_size.x, pixel_size.y)) * 0.1216216;
  gl_FragColor = c;
}
