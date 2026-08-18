<script lang="ts">
  type Props = {
    hue?: number;
    hoverIntensity?: number;
    rotateOnHover?: boolean;
    forceHoverState?: boolean;
    backgroundColor?: string;
    class?: string;
  };

  let {
    hue = 0,
    hoverIntensity = 0.2,
    rotateOnHover = true,
    forceHoverState = false,
    backgroundColor = "#000000",
    class: className = "",
  }: Props = $props();

  let current = $derived({
    hue,
    hoverIntensity,
    rotateOnHover,
    forceHoverState,
    backgroundColor,
  });

  function initializeOrb(canvasElement: HTMLCanvasElement) {
    let disposed = false;
    let cleanup: (() => void) | undefined;

    void import("ogl")
      .then((ogl) => {
        if (disposed) return;
        cleanup = mountOrb(canvasElement, ogl);
      })
      .catch((reason: unknown) => {
        console.warn("Orb WebGL renderer unavailable; using CSS fallback", reason);
      });

    return () => {
      disposed = true;
      cleanup?.();
    };
  }

  function mountOrb(
    canvasElement: HTMLCanvasElement,
    { Mesh, Program, Renderer, Triangle, Vec3 }: typeof import("ogl"),
  ) {
    const containerElement = canvasElement.parentElement;
    if (!containerElement) return;

    let renderer: InstanceType<typeof Renderer>;
    try {
      renderer = new Renderer({
        canvas: canvasElement,
        alpha: true,
        premultipliedAlpha: true,
        dpr: Math.min(window.devicePixelRatio || 1, 2),
      });
    } catch {
      return;
    }

    const gl = renderer.gl;
    gl.clearColor(0, 0, 0, 0);
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

    const colorVector = (color: string) => {
      if (/^#[0-9a-f]{6}$/i.test(color)) {
        return new Vec3(
          Number.parseInt(color.slice(1, 3), 16) / 255,
          Number.parseInt(color.slice(3, 5), 16) / 255,
          Number.parseInt(color.slice(5, 7), 16) / 255,
        );
      }
      return new Vec3(0, 0, 0);
    };

    const vertex = `precision highp float;
attribute vec2 position;
attribute vec2 uv;
varying vec2 vUv;
void main() { vUv = uv; gl_Position = vec4(position, 0.0, 1.0); }`;

    const fragment = `precision highp float;
uniform float iTime;
uniform vec3 iResolution;
uniform float hue;
uniform float hover;
uniform float rot;
uniform float hoverIntensity;
uniform vec3 backgroundColor;
varying vec2 vUv;
vec3 rgb2yiq(vec3 c) { float y = dot(c, vec3(0.299, 0.587, 0.114)); float i = dot(c, vec3(0.596, -0.274, -0.322)); float q = dot(c, vec3(0.211, -0.523, 0.312)); return vec3(y, i, q); }
vec3 yiq2rgb(vec3 c) { float r = c.x + 0.956 * c.y + 0.621 * c.z; float g = c.x - 0.272 * c.y - 0.647 * c.z; float b = c.x - 1.106 * c.y + 1.703 * c.z; return vec3(r, g, b); }
vec3 adjustHue(vec3 color, float hueDeg) { float hueRad = hueDeg * 3.14159265 / 180.0; vec3 yiq = rgb2yiq(color); float cosA = cos(hueRad); float sinA = sin(hueRad); float i = yiq.y * cosA - yiq.z * sinA; float q = yiq.y * sinA + yiq.z * cosA; yiq.y = i; yiq.z = q; return yiq2rgb(yiq); }
vec3 hash33(vec3 p3) { p3 = fract(p3 * vec3(0.1031, 0.11369, 0.13787)); p3 += dot(p3, p3.yxz + 19.19); return -1.0 + 2.0 * fract(vec3(p3.x + p3.y, p3.x + p3.z, p3.y + p3.z) * p3.zyx); }
float snoise3(vec3 p) { const float K1 = 0.333333333; const float K2 = 0.166666667; vec3 i = floor(p + (p.x + p.y + p.z) * K1); vec3 d0 = p - (i - (i.x + i.y + i.z) * K2); vec3 e = step(vec3(0.0), d0 - d0.yzx); vec3 i1 = e * (1.0 - e.zxy); vec3 i2 = 1.0 - e.zxy * (1.0 - e); vec3 d1 = d0 - (i1 - K2); vec3 d2 = d0 - (i2 - K1); vec3 d3 = d0 - 0.5; vec4 h = max(0.6 - vec4(dot(d0, d0), dot(d1, d1), dot(d2, d2), dot(d3, d3)), 0.0); vec4 n = h * h * h * h * vec4(dot(d0, hash33(i)), dot(d1, hash33(i + i1)), dot(d2, hash33(i + i2)), dot(d3, hash33(i + 1.0))); return dot(vec4(31.316), n); }
vec4 extractAlpha(vec3 colorIn) { float a = max(max(colorIn.r, colorIn.g), colorIn.b); return vec4(colorIn.rgb / (a + 1e-5), a); }
const vec3 baseColor1 = vec3(0.611765, 0.262745, 0.996078);
const vec3 baseColor2 = vec3(0.298039, 0.760784, 0.913725);
const vec3 baseColor3 = vec3(0.062745, 0.078431, 0.600000);
const float innerRadius = 0.6;
const float noiseScale = 0.65;
float light1(float intensity, float attenuation, float dist) { return intensity / (1.0 + dist * attenuation); }
float light2(float intensity, float attenuation, float dist) { return intensity / (1.0 + dist * dist * attenuation); }
vec4 draw(vec2 uv) {
  vec3 color1 = adjustHue(baseColor1, hue);
  vec3 color2 = adjustHue(baseColor2, hue);
  vec3 color3 = adjustHue(baseColor3, hue);
  float ang = atan(uv.y, uv.x);
  float len = length(uv);
  float invLen = len > 0.0 ? 1.0 / len : 0.0;
  float bgLuminance = dot(backgroundColor, vec3(0.299, 0.587, 0.114));
  float n0 = snoise3(vec3(uv * noiseScale, iTime * 0.5)) * 0.5 + 0.5;
  float r0 = mix(mix(innerRadius, 1.0, 0.4), mix(innerRadius, 1.0, 0.6), n0);
  float d0 = distance(uv, (r0 * invLen) * uv);
  float v0 = light1(1.0, 10.0, d0);
  v0 *= smoothstep(r0 * 1.05, r0, len);
  float innerFade = smoothstep(r0 * 0.8, r0 * 0.95, len);
  v0 *= mix(innerFade, 1.0, bgLuminance * 0.7);
  float cl = cos(ang + iTime * 2.0) * 0.5 + 0.5;
  float a = iTime * -1.0;
  vec2 pos = vec2(cos(a), sin(a)) * r0;
  float d = distance(uv, pos);
  float v1 = light2(1.5, 5.0, d);
  v1 *= light1(1.0, 50.0, d0);
  float v2 = smoothstep(1.0, mix(innerRadius, 1.0, n0 * 0.5), len);
  float v3 = smoothstep(innerRadius, mix(innerRadius, 1.0, 0.5), len);
  vec3 colBase = mix(color1, color2, cl);
  float fadeAmount = mix(1.0, 0.1, bgLuminance);
  vec3 darkCol = mix(color3, colBase, v0);
  darkCol = (darkCol + v1) * v2 * v3;
  darkCol = clamp(darkCol, 0.0, 1.0);
  vec3 lightCol = (colBase + v1) * mix(1.0, v2 * v3, fadeAmount);
  lightCol = mix(backgroundColor, lightCol, v0);
  lightCol = clamp(lightCol, 0.0, 1.0);
  vec3 finalCol = mix(darkCol, lightCol, bgLuminance);
  return extractAlpha(finalCol);
}
vec4 mainImage(vec2 fragCoord) {
  vec2 center = iResolution.xy * 0.5;
  float size = min(iResolution.x, iResolution.y);
  vec2 circleUv = (fragCoord - center) / size * 2.0;
  vec2 uv = circleUv;
  float angle = rot;
  float s = sin(angle);
  float c = cos(angle);
  uv = vec2(c * uv.x - s * uv.y, s * uv.x + c * uv.y);
  uv.x += hover * hoverIntensity * 0.1 * sin(uv.y * 10.0 + iTime);
  uv.y += hover * hoverIntensity * 0.1 * sin(uv.x * 10.0 + iTime);
  vec4 orb = draw(uv);
  float circleDistance = length(circleUv);
  float circleMask = 1.0 - smoothstep(0.965, 0.995, circleDistance);
  float radialLight = 1.0 - smoothstep(0.0, 1.0, circleDistance);
  vec3 circularBase = clamp(adjustHue(baseColor3, hue), 0.0, 1.0);
  orb.rgb = mix(circularBase * (0.52 + radialLight * 0.18), orb.rgb, 0.82);
  orb.a = circleMask;
  return orb;
}
void main() {
  vec2 fragCoord = vUv * iResolution.xy;
  vec4 color = mainImage(fragCoord);
  gl_FragColor = vec4(color.rgb * color.a, color.a);
}`;

    const geometry = new Triangle(gl);
    const program = new Program(gl, {
      vertex,
      fragment,
      uniforms: {
        iTime: { value: 0 },
        iResolution: {
          value: new Vec3(
            gl.canvas.width,
            gl.canvas.height,
            gl.canvas.width / Math.max(gl.canvas.height, 1),
          ),
        },
        hue: { value: hue },
        hover: { value: 0 },
        rot: { value: 0 },
        hoverIntensity: { value: hoverIntensity },
        backgroundColor: { value: colorVector(backgroundColor) },
      },
    });
    const mesh = new Mesh(gl, { geometry, program });

    const resize = () => {
      renderer.setSize(
        Math.max(containerElement.clientWidth, 1),
        Math.max(containerElement.clientHeight, 1),
      );
      program.uniforms.iResolution.value.set(
        gl.canvas.width,
        gl.canvas.height,
        gl.canvas.width / Math.max(gl.canvas.height, 1),
      );
    };
    const resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(containerElement);
    resize();

    let targetHover = 0;
    const handlePointerMove = (event: PointerEvent) => {
      const bounds = canvasElement.getBoundingClientRect();
      const size = Math.min(bounds.width, bounds.height);
      const x = ((event.clientX - bounds.left - bounds.width / 2) / size) * 2;
      const y = ((event.clientY - bounds.top - bounds.height / 2) / size) * 2;
      targetHover = Math.hypot(x, y) < 0.8 ? 1 : 0;
    };
    const handlePointerLeave = () => {
      targetHover = 0;
    };
    window.addEventListener("pointermove", handlePointerMove, { passive: true });
    document.documentElement.addEventListener("mouseleave", handlePointerLeave);

    let frame = 0;
    let lastTime = performance.now();
    let currentRotation = 0;
    const render = (time: number) => {
      const delta = Math.min((time - lastTime) * 0.001, 0.1);
      lastTime = time;
      const effectiveHover = current.forceHoverState ? 1 : targetHover;

      program.uniforms.iTime.value = time * 0.001;
      program.uniforms.hue.value = current.hue;
      program.uniforms.hoverIntensity.value = current.hoverIntensity;
      program.uniforms.hover.value +=
        (effectiveHover - program.uniforms.hover.value) * 0.1;
      if (current.rotateOnHover && effectiveHover > 0.5) {
        currentRotation += delta * 0.3;
      }
      program.uniforms.rot.value = currentRotation;
      program.uniforms.backgroundColor.value = colorVector(
        current.backgroundColor,
      );
      renderer.render({ scene: mesh });

      if (!reducedMotion.matches && !document.hidden) {
        frame = requestAnimationFrame(render);
      }
    };

    const resume = () => {
      cancelAnimationFrame(frame);
      lastTime = performance.now();
      frame = requestAnimationFrame(render);
    };
    const handleVisibilityChange = () => {
      if (document.hidden) cancelAnimationFrame(frame);
      else resume();
    };
    const handleMotionChange = () => resume();
    document.addEventListener("visibilitychange", handleVisibilityChange);
    reducedMotion.addEventListener("change", handleMotionChange);
    resume();

    return () => {
      cancelAnimationFrame(frame);
      resizeObserver.disconnect();
      window.removeEventListener("pointermove", handlePointerMove);
      document.documentElement.removeEventListener(
        "mouseleave",
        handlePointerLeave,
      );
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      reducedMotion.removeEventListener("change", handleMotionChange);
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    };
  }
</script>

<div
  class={["orb", className]}
  style:--orb-background={backgroundColor}
  style:--orb-hue={`${hue}deg`}
  aria-hidden="true"
>
  <canvas {@attach initializeOrb} aria-hidden="true"></canvas>
</div>

<style>
  .orb {
    position: relative;
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    overflow: hidden;
    background: var(--orb-background);
  }

  .orb::before {
    content: "";
    position: absolute;
    width: min(76vmin, 760px);
    aspect-ratio: 1;
    border-radius: 50%;
    background:
      radial-gradient(circle at 35% 30%, hsl(var(--orb-hue) 90% 82% / 0.4), transparent 24%),
      radial-gradient(circle at 62% 60%, hsl(calc(var(--orb-hue) + 75deg) 82% 58% / 0.34), transparent 52%),
      radial-gradient(circle, hsl(var(--orb-hue) 72% 32% / 0.22), transparent 68%);
    filter: blur(30px);
    opacity: 0.55;
    animation: orb-fallback-pulse 7s ease-in-out infinite alternate;
  }

  .orb canvas {
    position: relative;
    z-index: 1;
    width: 100%;
    height: 100%;
    display: block;
  }

  @keyframes orb-fallback-pulse {
    from {
      opacity: 0.4;
      transform: scale(0.92) rotate(-3deg);
    }
    to {
      opacity: 0.62;
      transform: scale(1.06) rotate(3deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .orb::before {
      animation: none;
    }
  }
</style>
