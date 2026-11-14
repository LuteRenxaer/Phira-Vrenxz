#version 100
// 噪域 Glitch：等价移植 Phigros GlitchDog（DesultorySignals 故障后处理）
// GlitchMap（硬边噪点）驱动撕裂/错位，DisplaceMap（软渐变）驱动 UV 扭曲
precision highp float;

varying lowp vec2 uv;
uniform sampler2D screenTexture;
uniform sampler2D glitchMap;
uniform sampler2D displaceMap;
uniform float time;
uniform vec2 screenSize;

uniform float noiseFrameCount; // %11.0%
uniform float noiseMoveSpeed; // %2.0%
uniform float noiseEvolveSpeed; // %2.0%
uniform vec2 noiseScale; // %0.3,0.36%
uniform float scale; // %1.8%
uniform float seamBlend; // %0.02%
uniform float displaceStrength; // %0.2%
uniform vec2 displaceMapOffset; // %0.11,0%
uniform float displaceMapScale; // %1.0%
uniform float displaceSpeed; // %1.0%
uniform float glitchMapBrightness; // %0.0%
uniform float glitchMapContrast; // %3.5%
uniform vec2 glitchMapScale; // %5.6,1%
uniform float centerMaskRadius; // %0.094%
uniform float centerMaskSmoothness; // %0.07%
uniform float edgeMaskRadius; // %0.12%
uniform float edgeMaskSmoothness; // %0.41%
uniform vec4 centerColor; // %0,0.29,0.60,1%
uniform vec4 edgeColor; // %0.062,0.087,0.151,1%
uniform float colorBlend; // %0.776%

float my_trunc(float x) {
  return x < 0.0? -floor(-x): floor(x);
}

float rand(vec2 p) {
  return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}

void main() {
  // ---- 噪域帧演化 ----
  // noiseEvolveSpeed：演化速度（切帧），noiseMoveSpeed：沿 u 轴滚动的帧步长
  float evolve = my_trunc(time * noiseEvolveSpeed);
  float frame = my_trunc(time * noiseMoveSpeed);

  // GlitchMap 按 noiseFrameCount 帧横向切片滚动采样
  vec2 gUV = uv * glitchMapScale;
  gUV.x = fract(gUV.x / noiseFrameCount + frame / noiseFrameCount);
  gUV.y = fract(gUV.y + evolve * 0.05);
  float gm = texture2D(glitchMap, gUV).r;
  // 亮度/对比度
  gm = clamp((gm + glitchMapBrightness) * glitchMapContrast, 0.0, 1.0);

  // ---- DisplaceMap：软渐变驱动 UV 扭曲 ----
  vec2 duv = uv * displaceMapScale + displaceMapOffset + vec2(time * displaceSpeed * 0.01, 0.0);
  vec2 disp = texture2D(displaceMap, duv).rg - 0.5;
  vec2 warped = uv + disp * displaceStrength * 2.0;

  // ---- 行撕裂：按行随机横向偏移，幅度由 glitchMap 强度调制 ----
  float row = my_trunc(warped.y * 48.0) / 48.0;
  float jitter = (rand(vec2(row, frame)) - 0.5) * scale * gm * 0.04;
  // seamBlend：接缝处平滑过渡，避免硬边
  float seam = smoothstep(0.0, seamBlend, fract(warped.y * 48.0)) *
               smoothstep(1.0, 1.0 - seamBlend, fract(warped.y * 48.0));
  jitter *= mix(seam, 1.0, 1.0 - seamBlend);
  vec2 final_uv = warped + vec2(jitter, 0.0);

  // ---- 中心/边缘遮罩 ----
  vec2 center_uv = (final_uv - 0.5) * vec2(screenSize.x / max(screenSize.y, 1.0), 1.0);
  float dist = length(center_uv);
  float centerMask = 1.0 - smoothstep(centerMaskRadius - centerMaskSmoothness, centerMaskRadius, dist);
  float edgeMask = smoothstep(edgeMaskRadius, edgeMaskRadius + edgeMaskSmoothness, dist);

  // ---- 采样 + RGB 通道错位 ----
  vec4 color = texture2D(screenTexture, final_uv);
  vec2 chroma = vec2((gm - 0.5) * 0.02, 0.0);
  color.r = texture2D(screenTexture, final_uv + chroma).r;
  color.b = texture2D(screenTexture, final_uv - chroma).b;

  // ---- 中心/边缘染色 ----
  float tintAmount = clamp(centerMask + edgeMask, 0.0, 1.0);
  vec3 tint = mix(centerColor.rgb, edgeColor.rgb, edgeMask / max(centerMask + edgeMask, 1e-4));
  color.rgb = mix(color.rgb, color.rgb * tint, colorBlend * tintAmount);

  gl_FragColor = color;
}
