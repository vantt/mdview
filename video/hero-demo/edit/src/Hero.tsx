import { Audio, Video } from "@remotion/media";
import { AbsoluteFill, Easing, Freeze, interpolate, Sequence, staticFile, useCurrentFrame } from "remotion";
import { type Caption, type Click, FPS, frames, look, type Shot, shotSeconds } from "./data";

const SANS = "Geist, system-ui, sans-serif";
const MONO = "'Geist Mono', ui-monospace, monospace";

const Fonts = () => (
	<style>{`
		@font-face { font-family: Geist; src: url(${staticFile("fonts/Geist-Regular.ttf")}); font-weight: 400; }
		@font-face { font-family: Geist; src: url(${staticFile("fonts/Geist-Medium.ttf")}); font-weight: 500; }
		@font-face { font-family: Geist; src: url(${staticFile("fonts/Geist-SemiBold.ttf")}); font-weight: 600; }
		@font-face { font-family: 'Geist Mono'; src: url(${staticFile("fonts/GeistMono-Regular.ttf")}); }
	`}</style>
);

const ease = Easing.bezier(0.22, 1, 0.36, 1);

// ── Recording, with an optional hold on its last frame ─────────────────────────
const Recording = ({ shot }: { shot: Shot }) => {
	const rate = shot.rate ?? 1;
	const own = frames(((shot.to ?? 0) - (shot.from ?? 0)) / rate);
	const hold = frames(shot.freeze ?? 0);
	const video = (
		<Video
			src={staticFile(shot.src!)}
			trimBefore={frames(shot.from ?? 0)}
			playbackRate={rate}
			muted
			style={{ width: "100%", height: "100%", objectFit: "cover" }}
		/>
	);
	return (
		<AbsoluteFill style={{ background: look.stage }}>
			{own > 0 ? (
				<Sequence durationInFrames={own} premountFor={30}>
					{video}
				</Sequence>
			) : null}
			{hold > 0 ? (
				<Sequence from={own} durationInFrames={hold}>
					<Freeze frame={Math.max(0, own - 1)}>{video}</Freeze>
				</Sequence>
			) : null}
		</AbsoluteFill>
	);
};

// ── On-screen words: bottom band, one line or two, a key phrase in ember ──────
const CaptionLines = ({ caption, until }: { caption: Caption; until: number }) => {
	const frame = useCurrentFrame();
	const inP = interpolate(frame, [0, 8], [0, 1], { extrapolateRight: "clamp", easing: ease });
	const outP = interpolate(frame, [until - 6, until], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
	const twoRows = caption.lines.length === 2 && caption.lines[1].accent === "muted";
	return (
		<div
			style={{
				position: "absolute", left: 0, right: 0, bottom: 74, textAlign: "center",
				fontFamily: SANS, fontWeight: 500, fontSize: 54, letterSpacing: "-0.02em", lineHeight: 1.18,
				color: look.text, opacity: inP * outP, transform: `translateY(${(1 - inP) * 14}px)`,
				textShadow: "0 2px 18px rgba(0,0,0,0.65)",
			}}
		>
			{caption.lines.map((l, i) => (
				<span
					key={i}
					style={{
						display: twoRows ? "block" : "inline",
						color: l.accent === "ember" ? look.ember : l.accent === "muted" ? look.muted : look.text,
					}}
				>
					{l.text}
				</span>
			))}
		</div>
	);
};

const Captions = ({ shot, length }: { shot: Shot; length: number }) => (
	<>
		{/* A soft dark band so the words read over any recording. */}
		<AbsoluteFill style={{ background: "linear-gradient(to top, rgba(13,9,7,0.92) 0%, rgba(13,9,7,0.6) 16%, transparent 30%)" }} />
		{(shot.captions ?? []).map((c, i, all) => {
			const start = frames(c.at);
			const end = i + 1 < all.length ? frames(all[i + 1].at) : length;
			return (
				<Sequence key={i} from={start} durationInFrames={Math.max(1, end - start)} layout="none">
					<CaptionLines caption={c} until={end - start} />
				</Sequence>
			);
		})}
	</>
);

// ── Pointer and ember click ring (no recording here draws a pointer) ───────────
const Pointer = ({ x, y, press }: { x: number; y: number; press: number }) => (
	<svg
		width="44" height="44" viewBox="0 0 24 24"
		style={{ position: "absolute", left: x - 6, top: y - 3, transform: `scale(${1 - press * 0.12})`, transformOrigin: "6px 3px", filter: "drop-shadow(0 3px 6px rgba(0,0,0,0.6))" }}
	>
		<path d="M5 3 L5 19 L9.5 14.8 L12.6 21 L15.4 19.7 L12.4 13.6 L18.5 13.6 Z" fill={look.text} stroke="#140e0b" strokeWidth="1.3" strokeLinejoin="round" />
	</svg>
);

const ClickAt = ({ click, prev }: { click: Click; prev?: Click }) => {
	const frame = useCurrentFrame(); // 0 = 12 frames before the click
	const glide = interpolate(frame, [0, 12], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: ease });
	const fromX = prev ? prev.x : click.x + 140;
	const fromY = prev ? prev.y : click.y + 90;
	const x = fromX + (click.x - fromX) * glide;
	const y = fromY + (click.y - fromY) * glide;
	const t = frame - 12;
	const press = interpolate(t, [0, 3, 8], [0, 1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
	const ring = interpolate(t, [0, 16], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: ease });
	const ringOpacity = t < 0 ? 0 : 1 - ring;
	const appear = prev ? 1 : interpolate(frame, [0, 5], [0, 1], { extrapolateRight: "clamp" });
	return (
		<AbsoluteFill style={{ opacity: appear }}>
			<div
				style={{
					position: "absolute", left: click.x - 46, top: click.y - 46, width: 92, height: 92, borderRadius: "50%",
					border: `3px solid ${look.emberHi}`, boxShadow: `0 0 28px ${look.emberHi}`,
					transform: `scale(${0.35 + ring * 0.9})`, opacity: ringOpacity,
				}}
			/>
			<div
				style={{
					position: "absolute", left: click.x - 9, top: click.y - 9, width: 18, height: 18, borderRadius: "50%",
					background: look.emberHi, opacity: t < 0 ? 0 : interpolate(t, [0, 12], [0.95, 0], { extrapolateRight: "clamp" }),
				}}
			/>
			<Pointer x={x} y={y} press={press} />
			<Sequence from={12} layout="none">
				<Audio src={staticFile("audio/click.wav")} volume={0.55} />
			</Sequence>
		</AbsoluteFill>
	);
};

const Clicks = ({ clicks, length }: { clicks: Click[]; length: number }) => (
	<>
		{clicks.map((c, i) => {
			const from = Math.max(0, frames(c.at) - 12);
			const next = clicks[i + 1];
			const end = next ? Math.max(from + 1, frames(next.at) - 12) : Math.min(length, frames(c.at) + 24);
			return (
				<Sequence key={i} from={from} durationInFrames={Math.max(1, end - from)} layout="none">
					<ClickAt click={c} prev={i > 0 ? clicks[i - 1] : undefined} />
				</Sequence>
			);
		})}
	</>
);

// ── Ember underline and glow on the agent's link (scene 3) ─────────────────────
const LinkGlow = ({ x, y, w, left }: { x: number; y: number; w: number; left?: number }) => {
	const frame = useCurrentFrame();
	const grow = interpolate(frame, [0, 10], [0, 1], { extrapolateRight: "clamp", easing: ease });
	const x0 = left ?? x - w / 2;
	return (
		<>
			<div
				style={{
					position: "absolute", left: x0 - 10, top: y - 26, width: (w + 20) * grow, height: 52, borderRadius: 10,
					background: "rgba(233,89,12,0.16)", boxShadow: "0 0 40px rgba(255,122,46,0.35)", mixBlendMode: "screen",
				}}
			/>
			<div style={{ position: "absolute", left: x0, top: y + 20, width: w * grow, height: 3, background: look.emberHi, boxShadow: `0 0 12px ${look.emberHi}` }} />
		</>
	);
};

// ── Counter of real seconds during the sped-up hunt ────────────────────────────
const Timer = ({ rate }: { rate: number }) => {
	const frame = useCurrentFrame();
	const seconds = Math.floor((frame / FPS) * rate);
	return (
		<div
			style={{
				position: "absolute", right: 56, top: 44, padding: "10px 20px", borderRadius: 999,
				background: "rgba(20,14,11,0.85)", border: `1px solid ${look.dim}`, color: look.text,
				fontFamily: MONO, fontSize: 30, display: "flex", gap: 14, alignItems: "center",
			}}
		>
			<span style={{ width: 12, height: 12, borderRadius: "50%", background: look.ember, boxShadow: `0 0 12px ${look.ember}` }} />
			{`0:${String(seconds).padStart(2, "0")}`}
			<span style={{ color: look.muted, fontSize: 24 }}>{`${rate}×`}</span>
		</div>
	);
};

// ── End card (style frame m7) ──────────────────────────────────────────────────
const Rings = () => (
	<svg viewBox="0 0 1920 1080" style={{ position: "absolute", inset: 0 }}>
		<g fill="none" stroke="rgba(255,236,222,0.07)" strokeWidth="1.5">
			<circle cx="960" cy="540" r="300" /><circle cx="960" cy="540" r="480" />
			<circle cx="960" cy="540" r="680" /><circle cx="960" cy="540" r="900" />
		</g>
	</svg>
);

const EndCard = () => {
	const frame = useCurrentFrame();
	const show = (start: number) => ({
		opacity: interpolate(frame, [start, start + 10], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }),
		transform: `translateY(${interpolate(frame, [start, start + 12], [16, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: ease })}px)`,
	});
	return (
		<AbsoluteFill style={{ background: `radial-gradient(1200px 800px at 50% 45%, #171009 0%, ${look.stage} 70%)`, color: look.text, fontFamily: SANS }}>
			<Rings />
			<AbsoluteFill style={{ alignItems: "center", justifyContent: "center", flexDirection: "column" }}>
				<div style={{ ...show(0), fontSize: 168, fontWeight: 600, letterSpacing: "-0.055em", display: "flex", alignItems: "center", gap: 26 }}>
					mdview
					<span style={{ width: 30, height: 30, borderRadius: "50%", background: look.ember, boxShadow: "0 0 50px rgba(233,89,12,0.8)", marginTop: 40 }} />
				</div>
				<div style={{ ...show(8), marginTop: 22, fontSize: 46, color: look.muted, letterSpacing: "-0.015em" }}>
					<b style={{ color: look.text, fontWeight: 500 }}>Click, read and fix</b> the docs your AI agent writes.
				</div>
				<div style={{ ...show(16), marginTop: 64, padding: "20px 36px", borderRadius: 14, border: "1px solid rgba(255,236,222,0.16)", background: "rgba(26,18,14,0.8)", fontFamily: MONO, fontSize: 28 }}>
					<div><span style={{ color: look.ember, marginRight: 14 }}>$</span>curl -fsSL https://raw.githubusercontent.com/vantt/mdview/main/install.sh | sh</div>
					<div style={{ marginTop: 8 }}><span style={{ color: look.ember, marginRight: 14 }}>$</span>mdview doctor --fix</div>
				</div>
				<div style={{ ...show(24), marginTop: 30, fontFamily: MONO, fontSize: 24, color: look.dim }}>
					Single Rust binary · MIT · github.com/vantt/mdview
				</div>
			</AbsoluteFill>
		</AbsoluteFill>
	);
};

// ── One shot, and the whole cut ────────────────────────────────────────────────
const ShotView = ({ shot }: { shot: Shot }) => {
	const length = frames(shotSeconds(shot));
	if (!shot.src) return <EndCard />;
	return (
		<AbsoluteFill>
			<Recording shot={shot} />
			{shot.linkGlow ? (
				<Sequence from={frames(shot.linkGlow.at)} layout="none">
					<LinkGlow {...shot.linkGlow} />
				</Sequence>
			) : null}
			{shot.timer ? <Timer rate={shot.timer.rate} /> : null}
			<Clicks clicks={shot.clicks ?? []} length={length} />
			{(shot.sfx ?? []).map((e, i) => (
				<Sequence key={i} from={frames(e.at)} layout="none">
					<Audio src={staticFile(e.src)} volume={e.volume ?? 0.5} />
				</Sequence>
			))}
			<Captions shot={shot} length={length} />
		</AbsoluteFill>
	);
};

// Music: John Bartmann, "Nova Serenade", CC0, from
// https://commons.wikimedia.org/wiki/File:John_Bartmann_-_Nova_Serenade.ogg (copied into public/audio/).
// Chosen by ear from seven licensed candidates (out/music-previews/).
const MUSIC = "audio/nova-serenade.ogg";
const MUSIC_VOLUME = 0.4;

const Music = ({ length }: { length: number }) => (
	<Audio
		src={staticFile(MUSIC)}
		volume={(f) =>
			MUSIC_VOLUME *
			interpolate(f, [0, 15, length - 45, length], [0, 1, 1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })
		}
	/>
);

export const Hero = ({ cut }: { cut: Shot[] }) => {
	let at = 0;
	const total = cut.reduce((sum, s) => sum + frames(shotSeconds(s)), 0);
	return (
		<AbsoluteFill style={{ background: look.stage }}>
			<Fonts />
			<Music length={total} />
			{cut.map((shot) => {
				const from = at;
				const length = frames(shotSeconds(shot));
				at += length;
				return (
					<Sequence key={shot.id} from={from} durationInFrames={length} premountFor={45}>
						<ShotView shot={shot} />
					</Sequence>
				);
			})}
		</AbsoluteFill>
	);
};
