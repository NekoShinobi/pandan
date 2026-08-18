<script lang="ts">
  type Offset = { x?: number | string; y?: number | string };
  type AnimationType = "rotate" | "rotate3d" | "hover";
  const MAX_FRAME_INTERVAL_MS = 1_000 / 60;

  type Props = {
    intensity?: number;
    speed?: number;
    animationType?: AnimationType;
    colors?: string[];
    distort?: number;
    paused?: boolean;
    offset?: Offset;
    hoverDampness?: number;
    rayCount?: number;
    mixBlendMode?: string | "none";
    class?: string;
  };

  let {
    intensity = 2,
    speed = 0.5,
    animationType = "rotate3d",
    colors,
    distort = 0,
    paused = false,
    offset = { x: 0, y: 0 },
    hoverDampness = 0,
    rayCount = 0,
    mixBlendMode = "lighten",
    class: className = "",
  }: Props = $props();

  let current = $derived({
    intensity,
    speed,
    animationType,
    colors,
    distort,
    paused,
    offset,
    hoverDampness,
    rayCount,
    mixBlendMode,
  });

  const vertexShader = `#version 300 es
in vec2 position;
in vec2 uv;
out vec2 vUv;
void main() { vUv = uv; gl_Position = vec4(position, 0.0, 1.0); }`;

  const fragmentShader = `#version 300 es
precision highp float;
precision highp int;
out vec4 fragColor;
uniform vec2 uResolution;
uniform float uTime;
uniform float uIntensity;
uniform float uSpeed;
uniform int uAnimType;
uniform vec2 uMouse;
uniform int uColorCount;
uniform float uDistort;
uniform vec2 uOffset;
uniform sampler2D uGradient;
uniform float uNoiseAmount;
uniform int uRayCount;
float hash21(vec2 p) { p = floor(p); float f = 52.9829189 * fract(dot(p, vec2(0.065, 0.005))); return fract(f); }
mat2 rot30() { return mat2(0.8, -0.5, 0.5, 0.8); }
float layeredNoise(vec2 fragPx) {
  vec2 p = mod(fragPx + vec2(uTime * 30.0, -uTime * 21.0), 1024.0);
  vec2 q = rot30() * p;
  float n = 0.0;
  n += 0.40 * hash21(q);
  n += 0.25 * hash21(q * 2.0 + 17.0);
  n += 0.20 * hash21(q * 4.0 + 47.0);
  n += 0.10 * hash21(q * 8.0 + 113.0);
  n += 0.05 * hash21(q * 16.0 + 191.0);
  return n;
}
vec3 rayDir(vec2 frag, vec2 res, vec2 offset, float dist) { float focal = res.y * max(dist, 1e-3); return normalize(vec3(2.0 * (frag - offset) - res, focal)); }
float edgeFade(vec2 frag, vec2 res, vec2 offset) {
  vec2 toC = frag - 0.5 * res - offset;
  float r = length(toC) / (0.5 * min(res.x, res.y));
  float x = clamp(r, 0.0, 1.0);
  float q = x * x * x * (x * (x * 6.0 - 15.0) + 10.0);
  float s = q * 0.5;
  s = pow(s, 1.5);
  float tail = 1.0 - pow(1.0 - s, 2.0);
  s = mix(s, tail, 0.2);
  float dn = (layeredNoise(frag * 0.15) - 0.5) * 0.0015 * s;
  return clamp(s + dn, 0.0, 1.0);
}
mat3 rotX(float a) { float c = cos(a), s = sin(a); return mat3(1.0,0.0,0.0, 0.0,c,-s, 0.0,s,c); }
mat3 rotY(float a) { float c = cos(a), s = sin(a); return mat3(c,0.0,s, 0.0,1.0,0.0, -s,0.0,c); }
mat3 rotZ(float a) { float c = cos(a), s = sin(a); return mat3(c,-s,0.0, s,c,0.0, 0.0,0.0,1.0); }
vec3 sampleGradient(float t) { t = clamp(t, 0.0, 1.0); return texture(uGradient, vec2(t, 0.5)).rgb; }
vec2 rot2(vec2 v, float a) { float s = sin(a), c = cos(a); return mat2(c, -s, s, c) * v; }
float bendAngle(vec3 q, float t) {
  return 0.8 * sin(q.x * 0.55 + t * 0.6) + 0.7 * sin(q.y * 0.50 - t * 0.5) + 0.6 * sin(q.z * 0.60 + t * 0.7);
}
void main() {
  vec2 frag = gl_FragCoord.xy;
  float t = uTime * uSpeed;
  float jitterAmp = 0.1 * clamp(uNoiseAmount, 0.0, 1.0);
  vec3 dir = rayDir(frag, uResolution, uOffset, 1.0);
  float marchT = 0.0;
  vec3 col = vec3(0.0);
  float n = layeredNoise(frag);
  vec4 c = cos(t * 0.2 + vec4(0.0, 33.0, 11.0, 0.0));
  mat2 M2 = mat2(c.x, c.y, c.z, c.w);
  float amp = clamp(uDistort, 0.0, 50.0) * 0.15;
  mat3 rot3dMat = mat3(1.0);
  if (uAnimType == 1) {
    vec3 ang = vec3(t * 0.31, t * 0.21, t * 0.17);
    rot3dMat = rotZ(ang.z) * rotY(ang.y) * rotX(ang.x);
  }
  mat3 hoverMat = mat3(1.0);
  if (uAnimType == 2) {
    vec2 m = uMouse * 2.0 - 1.0;
    vec3 ang = vec3(m.y * 0.6, m.x * 0.6, 0.0);
    hoverMat = rotY(ang.y) * rotX(ang.x);
  }
  for (int i = 0; i < 44; ++i) {
    vec3 P = marchT * dir;
    P.z -= 2.0;
    float rad = length(P);
    vec3 Pl = P * (10.0 / max(rad, 1e-6));
    if (uAnimType == 0) Pl.xz *= M2;
    else if (uAnimType == 1) Pl = rot3dMat * Pl;
    else Pl = hoverMat * Pl;
    float surfaceDistance = max(rad - 0.3, 0.0);
    float stepLen = min(surfaceDistance, n * jitterAmp) + 0.1;
    float coreFade = smoothstep(0.75, 1.35, rad);
    float grow = smoothstep(0.35, 3.0, marchT);
    float a1 = amp * grow * bendAngle(Pl * 0.6, t);
    float a2 = 0.5 * amp * grow * bendAngle(Pl.zyx * 0.5 + 3.1, t * 0.9);
    vec3 Pb = Pl;
    Pb.xz = rot2(Pb.xz, a1);
    Pb.xy = rot2(Pb.xy, a2);
    float rayPattern = smoothstep(0.5, 0.7, sin(Pb.x + cos(Pb.y) * cos(Pb.z)) * sin(Pb.z + sin(Pb.y) * cos(Pb.x + t)));
    if (uRayCount > 0) {
      float ang = atan(Pb.y, Pb.x);
      float comb = 0.5 + 0.5 * cos(float(uRayCount) * ang);
      comb = pow(comb, 3.0);
      rayPattern *= smoothstep(0.15, 0.95, comb);
    }
    vec3 spectralDefault = 1.0 + vec3(cos(marchT * 3.0), cos(marchT * 3.0 + 1.0), cos(marchT * 3.0 + 2.0));
    float saw = fract(marchT * 0.25);
    float tRay = saw * saw * (3.0 - 2.0 * saw);
    vec3 userGradient = 2.0 * sampleGradient(tRay);
    vec3 spectral = (uColorCount > 0) ? userGradient : spectralDefault;
    vec3 base = (0.05 / (0.4 + stepLen)) * smoothstep(5.0, 0.0, rad) * coreFade * spectral;
    col += base * rayPattern;
    marchT += stepLen;
  }
  float radialReveal = edgeFade(frag, uResolution, uOffset);
  col *= mix(0.24, 0.70, radialReveal);
  col *= uIntensity;
  vec3 outCol = clamp(col, 0.0, 1.0);
  fragColor = vec4(outCol, max(max(outCol.r, outCol.g), outCol.b));
}`;

  function initializeBurst(canvasElement: HTMLCanvasElement) {
    let disposed = false;
    let cleanup: (() => void) | undefined;

    void import("ogl")
      .then((ogl) => {
        if (disposed) return;
        cleanup = mountBurst(canvasElement, ogl);
      })
      .catch((reason: unknown) => {
        console.warn(
          "PrismaticBurst WebGL renderer unavailable; using CSS fallback",
          reason,
        );
      });

    return () => {
      disposed = true;
      cleanup?.();
    };
  }

  function mountBurst(
    canvasElement: HTMLCanvasElement,
    { Mesh, Program, Renderer, Texture, Triangle }: typeof import("ogl"),
  ) {
    const containerElement = canvasElement.parentElement;
    if (!containerElement) return;

    let renderer: InstanceType<typeof Renderer>;
    try {
      renderer = new Renderer({
        canvas: canvasElement,
        dpr: Math.min(window.devicePixelRatio || 1, 2),
        alpha: true,
        premultipliedAlpha: false,
        antialias: false,
      });
    } catch {
      return;
    }

    const gl = renderer.gl;
    gl.clearColor(0, 0, 0, 0);
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

    const white = new Uint8Array([255, 255, 255, 255]);
    const gradientTexture = new Texture(gl, {
      image: white,
      width: 1,
      height: 1,
      generateMipmaps: false,
      flipY: false,
    });
    gradientTexture.minFilter = gl.LINEAR;
    gradientTexture.magFilter = gl.LINEAR;
    gradientTexture.wrapS = gl.CLAMP_TO_EDGE;
    gradientTexture.wrapT = gl.CLAMP_TO_EDGE;

    const program = new Program(gl, {
      vertex: vertexShader,
      fragment: fragmentShader,
      uniforms: {
        uResolution: { value: [1, 1] },
        uTime: { value: 0 },
        uIntensity: { value: intensity },
        uSpeed: { value: speed },
        uAnimType: { value: 1 },
        uMouse: { value: [0.5, 0.5] },
        uColorCount: { value: 0 },
        uDistort: { value: distort },
        uOffset: { value: [0, 0] },
        uGradient: { value: gradientTexture },
        uNoiseAmount: { value: 0.8 },
        uRayCount: { value: rayCount },
      },
    });
    const triangle = new Triangle(gl);
    const mesh = new Mesh(gl, { geometry: triangle, program });

    const resize = () => {
      renderer.setSize(
        Math.max(containerElement.clientWidth, 1),
        Math.max(containerElement.clientHeight, 1),
      );
      program.uniforms.uResolution.value = [
        gl.drawingBufferWidth,
        gl.drawingBufferHeight,
      ];
    };
    const resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(containerElement);
    resize();

    const mouseTarget: [number, number] = [0.5, 0.5];
    const mouseSmooth: [number, number] = [0.5, 0.5];
    const handlePointerMove = (event: PointerEvent) => {
      const bounds = containerElement.getBoundingClientRect();
      mouseTarget[0] = Math.min(
        Math.max((event.clientX - bounds.left) / Math.max(bounds.width, 1), 0),
        1,
      );
      mouseTarget[1] = Math.min(
        Math.max((event.clientY - bounds.top) / Math.max(bounds.height, 1), 0),
        1,
      );
    };
    window.addEventListener("pointermove", handlePointerMove, {
      passive: true,
    });

    const animationTypeMap: Record<AnimationType, number> = {
      rotate: 0,
      rotate3d: 1,
      hover: 2,
    };
    let gradientSignature = "";

    const parseColor = (color: string): [number, number, number] => {
      let hex = color.trim().replace(/^#/, "");
      if (hex.length === 3) {
        hex = hex
          .split("")
          .map((value) => value + value)
          .join("");
      }
      if (!/^[0-9a-f]{6}$/i.test(hex)) return [255, 255, 255];
      return [
        Number.parseInt(hex.slice(0, 2), 16),
        Number.parseInt(hex.slice(2, 4), 16),
        Number.parseInt(hex.slice(4, 6), 16),
      ];
    };

    const pixels = (value: number | string | undefined) => {
      if (typeof value === "number") return value;
      const parsed = Number.parseFloat(value ?? "0");
      return Number.isFinite(parsed) ? parsed : 0;
    };

    const updateGradient = () => {
      const palette = current.colors?.slice(0, 64) ?? [];
      const signature = palette.join(",");
      if (signature === gradientSignature) return;
      gradientSignature = signature;

      if (palette.length === 0) {
        gradientTexture.image = white;
        gradientTexture.width = 1;
        gradientTexture.height = 1;
        gradientTexture.needsUpdate = true;
        program.uniforms.uColorCount.value = 0;
        return;
      }

      const data = new Uint8Array(palette.length * 4);
      palette.forEach((color, index) => {
        const [red, green, blue] = parseColor(color);
        data[index * 4] = red;
        data[index * 4 + 1] = green;
        data[index * 4 + 2] = blue;
        data[index * 4 + 3] = 255;
      });
      gradientTexture.image = data;
      gradientTexture.width = palette.length;
      gradientTexture.height = 1;
      gradientTexture.needsUpdate = true;
      program.uniforms.uColorCount.value = palette.length;
    };

    const applyProps = () => {
      updateGradient();
      program.uniforms.uIntensity.value = current.intensity;
      program.uniforms.uSpeed.value = current.speed;
      program.uniforms.uAnimType.value =
        animationTypeMap[current.animationType];
      program.uniforms.uDistort.value = current.distort;
      program.uniforms.uOffset.value = [
        pixels(current.offset.x),
        pixels(current.offset.y),
      ];
      program.uniforms.uRayCount.value = Math.max(
        0,
        Math.floor(current.rayCount),
      );
      canvasElement.style.mixBlendMode =
        current.mixBlendMode === "none" ? "" : current.mixBlendMode;
    };

    let frame = 0;
    let lastRenderTime = 0;
    let accumulatedTime = 0;
    let visible = false;

    const render = (time: number) => {
      const elapsed = lastRenderTime === 0 ? 0 : time - lastRenderTime;
      if (lastRenderTime > 0 && elapsed < MAX_FRAME_INTERVAL_MS) {
        frame = requestAnimationFrame(render);
        return;
      }

      const delta = Math.min(Math.max(elapsed, 0) * 0.001, 0.1);
      lastRenderTime = time;
      applyProps();
      if (!current.paused && !reducedMotion.matches) {
        accumulatedTime += delta;
      }

      if (!current.paused && !reducedMotion.matches) {
        const damping =
          0.02 + Math.min(Math.max(current.hoverDampness, 0), 1) * 0.5;
        const smoothing = 1 - Math.exp(-delta / damping);
        mouseSmooth[0] += (mouseTarget[0] - mouseSmooth[0]) * smoothing;
        mouseSmooth[1] += (mouseTarget[1] - mouseSmooth[1]) * smoothing;
      }
      program.uniforms.uMouse.value = mouseSmooth;
      program.uniforms.uTime.value = accumulatedTime;
      renderer.render({ scene: mesh });

      if (visible && !document.hidden && !reducedMotion.matches) {
        frame = requestAnimationFrame(render);
      }
    };

    const resume = () => {
      cancelAnimationFrame(frame);
      if (!visible || document.hidden) return;
      lastRenderTime = 0;
      frame = requestAnimationFrame(render);
    };
    const visibilityObserver = new IntersectionObserver(([entry]) => {
      visible = entry?.isIntersecting ?? false;
      resume();
    });
    visibilityObserver.observe(containerElement);
    const handleVisibilityChange = () => resume();
    const handleMotionChange = () => resume();
    document.addEventListener("visibilitychange", handleVisibilityChange);
    reducedMotion.addEventListener("change", handleMotionChange);

    return () => {
      cancelAnimationFrame(frame);
      resizeObserver.disconnect();
      visibilityObserver.disconnect();
      window.removeEventListener("pointermove", handlePointerMove);
      document.removeEventListener(
        "visibilitychange",
        handleVisibilityChange,
      );
      reducedMotion.removeEventListener("change", handleMotionChange);
      if (gradientTexture.texture) gl.deleteTexture(gradientTexture.texture);
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    };
  }
</script>

<div
  class={["prismatic-burst", className]}
  aria-hidden="true"
>
  <canvas {@attach initializeBurst} aria-hidden="true"></canvas>
</div>

<style>
  .prismatic-burst {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: #000;
  }

  .prismatic-burst canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
