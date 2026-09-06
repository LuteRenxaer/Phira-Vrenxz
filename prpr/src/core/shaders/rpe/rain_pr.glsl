#version 100
precision highp float;

varying vec2 uv;          // 材质 UV
uniform vec2 screenSize;  // 屏幕大小
uniform sampler2D screenTexture;  // 屏幕材质
uniform float time;       // 时间，以秒为单位
uniform float density; // %500%
uniform float width; // %0.0005%
uniform vec4 rainColor; // %0.5, 0.5, 1.0, 0.5%
uniform float height; // %0.02%

// 随机生成函数，用于生成雨滴的位置和分布
float random(vec2 st) {
    return fract(sin(dot(st.xy, vec2(12.9898, 78.233))) * 43758.5453123);
}

// 生成雨滴的位置
vec2 generateDropPosition(float index, float timeOffset) {
    float x = random(vec2(index, time + timeOffset + 10.0)) * screenSize.x; // 随机横向位置
    float y = mod(random(vec2(index, timeOffset)) * screenSize.y - time * 2000.0, screenSize.y); // 雨滴竖向下落
    return vec2(x, y);
}

// 主程序
void main() {
    vec2 uvCoord = uv;  // 归一化屏幕坐标
    vec4 bgColor = texture2D(screenTexture, uvCoord);

    float dropWidth = width;   // 雨滴的宽度
    float dropHeight = height; // 雨滴的高度

    // 遍历生成 density 个雨滴
    float dropPresent = 0.0;
    for (float i = 0.0; i < density; i += 1.0) {
        vec2 dropPos = generateDropPosition(i, i * 10.0);

        // 检查当前像素是否属于该雨滴
        if (abs(gl_FragCoord.x - dropPos.x) < dropWidth * screenSize.x && abs(gl_FragCoord.y - dropPos.y) < dropHeight * screenSize.y) {
            dropPresent += 1.0;  // 如果当前像素属于雨滴，计数加1
        }
    }

    // 如果当前像素属于任意雨滴，将雨滴颜色叠加到背景上
    if (dropPresent > 0.0) {
        vec4 rainOverlay = rainColor;
        gl_FragColor = mix(bgColor, rainOverlay, rainOverlay.a); // 将雨滴叠加到背景上
        gl_FragColor.a = 1.0;
    } else {
        gl_FragColor = bgColor;  // 如果不属于雨滴，保持背景图片不变
        gl_FragColor.a = 1.0;
    }
}
