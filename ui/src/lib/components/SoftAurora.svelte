<script lang="ts">
  type Props = {
    speed?: number;
    scale?: number;
    brightness?: number;
    color1?: string;
    color2?: string;
    noiseFrequency?: number;
    noiseAmplitude?: number;
    bandHeight?: number;
    bandSpread?: number;
    octaveDecay?: number;
    layerOffset?: number;
    colorSpeed?: number;
    enableMouseInteraction?: boolean;
    mouseInfluence?: number;
    class?: string;
  };

  let {
    speed = 0.6,
    scale = 1.5,
    brightness = 1,
    color1 = "#f7f7f7",
    color2 = "#ff8a3d",
    noiseFrequency = 2.5,
    noiseAmplitude = 1,
    bandHeight = 0.5,
    bandSpread = 1,
    octaveDecay = 0.1,
    layerOffset = 0,
    colorSpeed = 1,
    enableMouseInteraction = true,
    mouseInfluence = 0.25,
    class: className = "",
  }: Props = $props();

  let current = $derived({
    speed,
    scale,
    brightness,
    color1,
    color2,
    noiseFrequency,
    noiseAmplitude,
    bandHeight,
    bandSpread,
    octaveDecay,
    layerOffset,
    colorSpeed,
    enableMouseInteraction,
    mouseInfluence,
  });

  function hexToVector(hex: string): [number, number, number] {
    const normalized = hex.replace("#", "").padEnd(6, "0").slice(0, 6);
    return [
      Number.parseInt(normalized.slice(0, 2), 16) / 255,
      Number.parseInt(normalized.slice(2, 4), 16) / 255,
      Number.parseInt(normalized.slice(4, 6), 16) / 255,
    ];
  }

  function initializeAurora(canvasElement: HTMLCanvasElement) {
		let disposed = false;
		let cleanup: (() => void) | undefined;

		void import("ogl")
      .then((ogl) => {
        if (disposed) return;
        cleanup = mountAurora(canvasElement, ogl);
      })
      .catch((reason: unknown) => {
        console.warn("SoftAurora WebGL renderer unavailable; using CSS fallback", reason);
      });

		return () => {
			disposed = true;
			cleanup?.();
		};
	}

	function mountAurora(
		canvasElement: HTMLCanvasElement,
		{ Mesh, Program, Renderer, Triangle }: typeof import("ogl")
	) {
		const containerElement = canvasElement.parentElement;
		if (!containerElement) return;

		let renderer: InstanceType<typeof Renderer>;
    try {
      renderer = new Renderer({
        canvas: canvasElement,
        alpha: true,
        premultipliedAlpha: false,
        dpr: Math.min(window.devicePixelRatio, 2),
      });
    } catch {
      return;
    }

    const gl = renderer.gl;
    gl.clearColor(0, 0, 0, 0);

    const targetMouse: [number, number] = [0.5, 0.5];
    const currentMouse: [number, number] = [0.5, 0.5];
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

    const handlePointerMove = (event: PointerEvent) => {
      targetMouse[0] = event.clientX / Math.max(window.innerWidth, 1);
      targetMouse[1] = 1 - event.clientY / Math.max(window.innerHeight, 1);
    };
    const handlePointerLeave = () => {
      targetMouse[0] = 0.5;
      targetMouse[1] = 0.5;
    };

    const vertex = `
attribute vec2 uv;
attribute vec2 position;
varying vec2 vUv;
void main() { vUv = uv; gl_Position = vec4(position, 0, 1); }`;

    const fragment = `precision highp float;
uniform float uTime;
uniform vec3 uResolution;
uniform float uSpeed;
uniform float uScale;
uniform float uBrightness;
uniform vec3 uColor1;
uniform vec3 uColor2;
uniform float uNoiseFreq;
uniform float uNoiseAmp;
uniform float uBandHeight;
uniform float uBandSpread;
uniform float uOctaveDecay;
uniform float uLayerOffset;
uniform float uColorSpeed;
uniform vec2 uMouse;
uniform float uMouseInfluence;
uniform bool uEnableMouse;
#define TAU 6.28318
vec3 gradientHash(vec3 p) {
  p = vec3(
    dot(p, vec3(127.1, 311.7, 234.6)),
    dot(p, vec3(269.5, 183.3, 198.3)),
    dot(p, vec3(169.5, 283.3, 156.9))
  );
  vec3 h = fract(sin(p) * 43758.5453123);
  float phi = acos(2.0 * h.x - 1.0);
  float theta = TAU * h.y;
  return vec3(cos(theta) * sin(phi), sin(theta) * cos(phi), cos(phi));
}
float quinticSmooth(float t) {
  float t2 = t * t;
  float t3 = t * t2;
  return 6.0 * t3 * t2 - 15.0 * t2 * t2 + 10.0 * t3;
}
vec3 cosineGradient(float t, vec3 a, vec3 b, vec3 c, vec3 d) {
  return a + b * cos(TAU * (c * t + d));
}
float perlin3D(float amplitude, float frequency, float px, float py, float pz) {
  float x = px * frequency;
  float y = py * frequency;
  float fx = floor(x); float fy = floor(y); float fz = floor(pz);
  float cx = ceil(x); float cy = ceil(y); float cz = ceil(pz);
  vec3 g000 = gradientHash(vec3(fx, fy, fz));
  vec3 g100 = gradientHash(vec3(cx, fy, fz));
  vec3 g010 = gradientHash(vec3(fx, cy, fz));
  vec3 g110 = gradientHash(vec3(cx, cy, fz));
  vec3 g001 = gradientHash(vec3(fx, fy, cz));
  vec3 g101 = gradientHash(vec3(cx, fy, cz));
  vec3 g011 = gradientHash(vec3(fx, cy, cz));
  vec3 g111 = gradientHash(vec3(cx, cy, cz));
  float d000 = dot(g000, vec3(x - fx, y - fy, pz - fz));
  float d100 = dot(g100, vec3(x - cx, y - fy, pz - fz));
  float d010 = dot(g010, vec3(x - fx, y - cy, pz - fz));
  float d110 = dot(g110, vec3(x - cx, y - cy, pz - fz));
  float d001 = dot(g001, vec3(x - fx, y - fy, pz - cz));
  float d101 = dot(g101, vec3(x - cx, y - fy, pz - cz));
  float d011 = dot(g011, vec3(x - fx, y - cy, pz - cz));
  float d111 = dot(g111, vec3(x - cx, y - cy, pz - cz));
  float sx = quinticSmooth(x - fx);
  float sy = quinticSmooth(y - fy);
  float sz = quinticSmooth(pz - fz);
  float lx00 = mix(d000, d100, sx);
  float lx10 = mix(d010, d110, sx);
  float lx01 = mix(d001, d101, sx);
  float lx11 = mix(d011, d111, sx);
  float ly0 = mix(lx00, lx10, sy);
  float ly1 = mix(lx01, lx11, sy);
  return amplitude * mix(ly0, ly1, sz);
}
float auroraGlow(float t, vec2 shift) {
  vec2 uv = gl_FragCoord.xy / uResolution.y;
  uv += shift;
  float noiseVal = 0.0;
  float frequency = uNoiseFreq;
  float amplitude = uNoiseAmp;
  vec2 samplePosition = uv * uScale;
  for (float i = 0.0; i < 3.0; i += 1.0) {
    noiseVal += perlin3D(amplitude, frequency, samplePosition.x, samplePosition.y, t);
    amplitude *= uOctaveDecay;
    frequency *= 2.0;
  }
  float yBand = uv.y * 10.0 - uBandHeight * 10.0;
  return 0.3 * max(exp(uBandSpread * (1.0 - 1.1 * abs(noiseVal + yBand))), 0.0);
}
void main() {
  vec2 uv = gl_FragCoord.xy / uResolution.xy;
  float t = uSpeed * 0.4 * uTime;
  vec2 shift = vec2(0.0);
  if (uEnableMouse) shift = (uMouse - 0.5) * uMouseInfluence;
  vec3 color = vec3(0.0);
  color += 0.99 * auroraGlow(t, shift) * cosineGradient(
    uv.x + uTime * uSpeed * 0.2 * uColorSpeed,
    vec3(0.5), vec3(0.5), vec3(1.0), vec3(0.3, 0.20, 0.20)
  ) * uColor1;
  color += 0.99 * auroraGlow(t + uLayerOffset, shift) * cosineGradient(
    uv.x + uTime * uSpeed * 0.1 * uColorSpeed,
    vec3(0.5), vec3(0.5), vec3(2.0, 1.0, 0.0), vec3(0.5, 0.20, 0.25)
  ) * uColor2;
  color *= uBrightness;
  float alpha = clamp(length(color), 0.0, 1.0);
  gl_FragColor = vec4(color, alpha);
}`;

    const geometry = new Triangle(gl);
    const program = new Program(gl, {
      vertex,
      fragment,
      uniforms: {
        uTime: { value: 0 },
        uResolution: { value: [1, 1, 1] },
        uSpeed: { value: speed },
        uScale: { value: scale },
        uBrightness: { value: brightness },
        uColor1: { value: hexToVector(color1) },
        uColor2: { value: hexToVector(color2) },
        uNoiseFreq: { value: noiseFrequency },
        uNoiseAmp: { value: noiseAmplitude },
        uBandHeight: { value: bandHeight },
        uBandSpread: { value: bandSpread },
        uOctaveDecay: { value: octaveDecay },
        uLayerOffset: { value: layerOffset },
        uColorSpeed: { value: colorSpeed },
        uMouse: { value: new Float32Array([0.5, 0.5]) },
        uMouseInfluence: { value: mouseInfluence },
        uEnableMouse: { value: enableMouseInteraction },
      },
    });
    const mesh = new Mesh(gl, { geometry, program });
    const resize = () => {
      renderer.setSize(
        Math.max(containerElement.offsetWidth, 1),
        Math.max(containerElement.offsetHeight, 1),
      );
      program.uniforms.uResolution.value = [
        gl.canvas.width,
        gl.canvas.height,
        gl.canvas.width / Math.max(gl.canvas.height, 1),
      ];
    };
    const resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(containerElement);
    resize();

    if (enableMouseInteraction) {
      window.addEventListener("pointermove", handlePointerMove, { passive: true });
      document.documentElement.addEventListener("mouseleave", handlePointerLeave);
    }

    let frame = 0;
    const render = (time: number) => {
      program.uniforms.uTime.value = time * 0.001;
      program.uniforms.uSpeed.value = current.speed;
      program.uniforms.uScale.value = current.scale;
      program.uniforms.uBrightness.value = current.brightness;
      program.uniforms.uColor1.value = hexToVector(current.color1);
      program.uniforms.uColor2.value = hexToVector(current.color2);
      program.uniforms.uNoiseFreq.value = current.noiseFrequency;
      program.uniforms.uNoiseAmp.value = current.noiseAmplitude;
      program.uniforms.uBandHeight.value = current.bandHeight;
      program.uniforms.uBandSpread.value = current.bandSpread;
      program.uniforms.uOctaveDecay.value = current.octaveDecay;
      program.uniforms.uLayerOffset.value = current.layerOffset;
      program.uniforms.uColorSpeed.value = current.colorSpeed;
      program.uniforms.uMouseInfluence.value = current.mouseInfluence;
      program.uniforms.uEnableMouse.value = current.enableMouseInteraction;

      if (current.enableMouseInteraction) {
        currentMouse[0] += 0.05 * (targetMouse[0] - currentMouse[0]);
        currentMouse[1] += 0.05 * (targetMouse[1] - currentMouse[1]);
        program.uniforms.uMouse.value[0] = currentMouse[0];
        program.uniforms.uMouse.value[1] = currentMouse[1];
      }

      renderer.render({ scene: mesh });
      if (!reducedMotion.matches && !document.hidden) {
        frame = requestAnimationFrame(render);
      }
    };

    const resume = () => {
      cancelAnimationFrame(frame);
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
      document.documentElement.removeEventListener("mouseleave", handlePointerLeave);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      reducedMotion.removeEventListener("change", handleMotionChange);
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    };
  }
</script>

<div
  class={["soft-aurora", className]}
  style:--soft-aurora-color-1={color1}
  style:--soft-aurora-color-2={color2}
  aria-hidden="true"
>
  <canvas {@attach initializeAurora} aria-hidden="true"></canvas>
</div>

<style>
  .soft-aurora {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: color-mix(in oklch, var(--soft-aurora-color-2) 14%, transparent);
  }

  .soft-aurora::before {
    content: "";
    position: absolute;
    inset: -28%;
    background:
      radial-gradient(
        ellipse at 22% 58%,
        color-mix(in oklch, var(--soft-aurora-color-1) 72%, transparent) 0%,
        transparent 48%
      ),
      radial-gradient(
        ellipse at 78% 42%,
        color-mix(in oklch, var(--soft-aurora-color-2) 76%, transparent) 0%,
        transparent 52%
      );
    filter: blur(46px) saturate(125%);
    opacity: 0.72;
    animation: soft-aurora-fallback 9s ease-in-out infinite alternate;
  }

  .soft-aurora canvas {
    position: relative;
    z-index: 1;
    width: 100%;
    height: 100%;
    display: block;
  }

  @keyframes soft-aurora-fallback {
    from {
      transform: translate3d(-5%, 3%, 0) scale(0.96) rotate(-2deg);
    }
    to {
      transform: translate3d(5%, -3%, 0) scale(1.08) rotate(2deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .soft-aurora::before {
      animation: none;
    }
  }
</style>
