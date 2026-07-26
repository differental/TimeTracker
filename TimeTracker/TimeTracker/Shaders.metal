#include <metal_stdlib>
using namespace metal;

static float hash21(float2 p) {
    p = fract(p * float2(234.34, 435.345));
    p += dot(p, p + 34.23);
    return fract(p.x * p.y);
}

static float noise(float2 p) {
    float2 i = floor(p);
    float2 f = fract(p);
    float2 u = f * f * (3.0 - 2.0 * f);
    float a = hash21(i);
    float b = hash21(i + float2(1.0, 0.0));
    float c = hash21(i + float2(0.0, 1.0));
    float d = hash21(i + float2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

static float fbm(float2 p) {
    float value = 0.0;
    float amplitude = 0.55;
    for (int octave = 0; octave < 5; octave++) {
        value += amplitude * noise(p);
        p = p * 2.03 + float2(11.7, 5.3);
        amplitude *= 0.5;
    }
    return value;
}

[[ stitchable ]] half4 aurora(
    float2 position,
    half4 color,
    float4 bounds,
    float time,
    half4 tint
) {
    float2 dimensions = max(bounds.zw, float2(1.0));
    float2 uv = (position - bounds.xy) / dimensions;
    float aspect = dimensions.x / dimensions.y;
    uv.x *= aspect;

    float t = time * 0.06;
    float2 q = float2(
        fbm(uv * 2.4 + t),
        fbm(uv * 2.4 - t * 1.3 + 5.2)
    );
    float2 r = float2(
        fbm(uv * 3.1 + q * 1.6 + t * 0.7),
        fbm(uv * 2.7 + q * 1.4 - t * 0.9 + 8.1)
    );
    float glow = fbm(uv * 2.2 + r * 1.8);

    float3 tint3 = float3(tint.rgb);
    // Shadows keep the tint's hue (tint² stays saturated when dark) over a
    // cool ink base, so warm activity colours read as atmosphere, not mud.
    float3 ink = float3(0.016, 0.020, 0.048);
    float3 deep = mix(ink, tint3 * tint3, 0.24);
    float3 veil = mix(tint3, float3(0.30, 0.38, 0.92), 0.45) * 0.40;
    float3 mid = tint3 * 0.62;
    float3 hot = min(tint3 * 1.3 + 0.2, 1.0);

    float3 result = mix(deep, veil, smoothstep(0.18, 0.55, glow));
    result = mix(result, mid, smoothstep(0.45, 0.82, glow));
    result = mix(result, hot, smoothstep(0.74, 0.98, glow) * 0.75);
    result *= mix(1.0, 0.55, smoothstep(0.15, 1.0, uv.y));
    result += (hash21(position + time) - 0.5) * 0.015;

    return half4(half3(result), color.a);
}
