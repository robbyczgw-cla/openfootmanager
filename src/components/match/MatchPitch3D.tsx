import { useEffect, useRef, useState } from "react";
import type { JSX } from "react";
import * as THREE from "three";

import type { MatchFrame } from "./matchFrame";

// ---------------------------------------------------------------------------
// MatchPitch3D — Three.js renderer for the SAME MatchFrame the 2D view uses.
//
// Model/view split pays off here: this is a second camera on the identical
// position stream (matchFrame.ts). Normalized 0..1 frame coords map onto a
// 105×68 pitch (X = length, Z = width, Y = up); the ball's `z` becomes height
// so passes/shots arc. Scene is built once; an rAF loop lerps tokens toward the
// latest frame for smooth motion regardless of React re-renders.
// ---------------------------------------------------------------------------

interface MatchPitch3DProps {
  frame: MatchFrame;
  homeColor: string;
  awayColor: string;
  homeName: string;
  awayName: string;
}

const L = 105; // pitch length (X)
const W = 68; // pitch width (Z)
const GK_COLOR = "#f59e0b";

const wx = (x: number): number => (x - 0.5) * L;
const wz = (y: number): number => (y - 0.5) * W;

function drawPitchTexture(home: string, away: string): THREE.CanvasTexture {
  const cw = 1050;
  const ch = 680;
  const c = document.createElement("canvas");
  c.width = cw;
  c.height = ch;
  const ctx = c.getContext("2d")!;
  const PAD = 28;
  const iw = cw - 2 * PAD;
  const ih = ch - 2 * PAD;
  // stripes
  const stripes = 12;
  const sw = iw / stripes;
  for (let i = 0; i < stripes; i++) {
    ctx.fillStyle = i % 2 === 0 ? "#2e8b4b" : "#298046";
    ctx.fillRect(PAD + i * sw, PAD, sw, ih);
  }
  ctx.strokeStyle = "rgba(255,255,255,0.8)";
  ctx.lineWidth = 3;
  ctx.strokeRect(PAD, PAD, iw, ih);
  // halfway + centre
  ctx.beginPath();
  ctx.moveTo(cw / 2, PAD);
  ctx.lineTo(cw / 2, ch - PAD);
  ctx.stroke();
  ctx.beginPath();
  ctx.arc(cw / 2, ch / 2, 82, 0, Math.PI * 2);
  ctx.stroke();
  // boxes
  const pbD = 150;
  const pbH = ih * 0.6;
  const pbY = PAD + (ih - pbH) / 2;
  const syD = 56;
  const syH = ih * 0.28;
  const syY = PAD + (ih - syH) / 2;
  ctx.strokeRect(PAD, pbY, pbD, pbH);
  ctx.strokeRect(PAD, syY, syD, syH);
  ctx.strokeRect(cw - PAD - pbD, pbY, pbD, pbH);
  ctx.strokeRect(cw - PAD - syD, syY, syD, syH);
  void home;
  void away;
  const tex = new THREE.CanvasTexture(c);
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.anisotropy = 8;
  return tex;
}

function makeNumberSprite(text: number): THREE.Sprite {
  const c = document.createElement("canvas");
  c.width = 64;
  c.height = 64;
  const ctx = c.getContext("2d")!;
  ctx.fillStyle = "#fff";
  ctx.font = "bold 40px sans-serif";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.strokeStyle = "rgba(0,0,0,0.6)";
  ctx.lineWidth = 5;
  ctx.strokeText(String(text), 32, 34);
  ctx.fillText(String(text), 32, 34);
  const tex = new THREE.CanvasTexture(c);
  const mat = new THREE.SpriteMaterial({ map: tex, depthTest: false, transparent: true });
  const sp = new THREE.Sprite(mat);
  sp.scale.set(4, 4, 1);
  return sp;
}

interface Token {
  group: THREE.Group;
  target: THREE.Vector3;
}

export default function MatchPitch3D({
  frame,
  homeColor,
  awayColor,
}: MatchPitch3DProps): JSX.Element {
  const mountRef = useRef<HTMLDivElement>(null);
  const frameRef = useRef<MatchFrame>(frame);
  frameRef.current = frame;
  const colorsRef = useRef({ home: homeColor, away: awayColor });
  colorsRef.current = { home: homeColor, away: awayColor };
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return;

    const width = mount.clientWidth || 800;
    const height = Math.min(width * 0.58, window.innerHeight * 0.6);

    const scene = new THREE.Scene();
    scene.background = new THREE.Color("#0b1220");

    const camera = new THREE.PerspectiveCamera(40, width / height, 0.1, 1000);
    camera.position.set(0, 60, W / 2 + 62);
    camera.lookAt(0, 0, -4);

    let renderer: THREE.WebGLRenderer;
    try {
      renderer = new THREE.WebGLRenderer({ antialias: true });
    } catch {
      setFailed(true);
      return;
    }
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(width, height);
    mount.appendChild(renderer.domElement);

    scene.add(new THREE.AmbientLight(0xffffff, 1.4));
    const dir = new THREE.DirectionalLight(0xffffff, 1.2);
    dir.position.set(-30, 60, 20);
    scene.add(dir);

    // pitch
    const pitchTex = drawPitchTexture(colorsRef.current.home, colorsRef.current.away);
    const pitch = new THREE.Mesh(
      new THREE.PlaneGeometry(L, W),
      new THREE.MeshStandardMaterial({ map: pitchTex }),
    );
    pitch.rotation.x = -Math.PI / 2;
    scene.add(pitch);

    // goals
    const goalMat = new THREE.MeshStandardMaterial({ color: "#ffffff" });
    for (const sx of [-1, 1]) {
      const goal = new THREE.Mesh(new THREE.BoxGeometry(1.5, 5, 16), goalMat);
      goal.position.set(sx * (L / 2 + 0.75), 2.5, 0);
      scene.add(goal);
    }

    // ball
    const ball = new THREE.Mesh(
      new THREE.SphereGeometry(0.85, 16, 16),
      new THREE.MeshStandardMaterial({ color: "#ffffff" }),
    );
    scene.add(ball);
    const ballTarget = new THREE.Vector3(0, 0.85, 0);

    // carrier ring
    const ring = new THREE.Mesh(
      new THREE.TorusGeometry(1.8, 0.25, 8, 24),
      new THREE.MeshBasicMaterial({ color: "#fde047" }),
    );
    ring.rotation.x = -Math.PI / 2;
    ring.visible = false;
    scene.add(ring);

    const tokens = new Map<string, Token>();

    function tokenFor(id: string, side: "Home" | "Away", role: string | undefined, num: number): Token {
      let tk = tokens.get(id);
      if (tk) return tk;
      const color =
        role === "GK" ? GK_COLOR : side === "Home" ? colorsRef.current.home : colorsRef.current.away;
      const group = new THREE.Group();
      const mat = new THREE.MeshStandardMaterial({ color });
      const body = new THREE.Mesh(new THREE.CylinderGeometry(0.85, 0.95, 2.2, 16), mat);
      body.position.y = 1.1;
      const head = new THREE.Mesh(new THREE.SphereGeometry(0.75, 16, 16), mat);
      head.position.y = 2.6;
      const sprite = makeNumberSprite(num);
      sprite.position.y = 4.2;
      group.add(body, head, sprite);
      scene.add(group);
      tk = { group, target: new THREE.Vector3() };
      tokens.set(id, tk);
      return tk;
    }

    let raf = 0;
    const animate = () => {
      const f = frameRef.current;
      const seen = new Set<string>();
      let carrier: THREE.Vector3 | null = null;
      for (const p of f.players) {
        seen.add(p.id);
        const tk = tokenFor(p.id, p.side, p.role, p.number);
        tk.target.set(wx(p.x), 0, wz(p.y));
        tk.group.visible = true;
        tk.group.position.lerp(tk.target, 0.25);
        if (p.has_ball) carrier = tk.group.position;
      }
      // hide tokens not in the frame (sent off)
      for (const [id, tk] of tokens) {
        if (!seen.has(id)) tk.group.visible = false;
      }
      ballTarget.set(wx(f.ball.x), 0.85 + f.ball.z * 45, wz(f.ball.y));
      ball.position.lerp(ballTarget, 0.3);
      if (carrier) {
        ring.visible = true;
        ring.position.set(carrier.x, 0.1, carrier.z);
      } else {
        ring.visible = false;
      }
      renderer.render(scene, camera);
      raf = requestAnimationFrame(animate);
    };
    animate();

    const onResize = () => {
      const w = mount.clientWidth || 800;
      const h = Math.min(w * 0.58, window.innerHeight * 0.6);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      renderer.setSize(w, h);
    };
    window.addEventListener("resize", onResize);

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", onResize);
      renderer.dispose();
      pitchTex.dispose();
      if (renderer.domElement.parentNode === mount) {
        mount.removeChild(renderer.domElement);
      }
    };
    // Build the scene once; live updates flow through frameRef.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div ref={mountRef} className="w-full overflow-hidden rounded-xl">
      {failed && (
        <div className="p-6 text-center text-sm text-gray-400">
          3D view unavailable — WebGL is not supported by this browser/device.
        </div>
      )}
    </div>
  );
}
