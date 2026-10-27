// The cut: which recording, which part, how fast, what is written on screen, where the clicks go.
// Times are seconds. Recordings are in public/out (../out): render.sh makes them.
import edit4 from "../public/out/scene4-read.clicks.json";
import edit5 from "../public/out/scene5-edit.clicks.json";
import edit6 from "../public/out/scene6-link.clicks.json";

export const FPS = 30;
export const frames = (s: number) => Math.round(s * FPS);

// The look of the style frames: dark stage, one ember accent.
export const look = {
	stage: "#0d0907",
	cocoa: "#1a120e",
	text: "#f4ede7",
	muted: "#9b8a7f",
	dim: "#5f524a",
	ember: "#e9590c",
	emberHi: "#ff7a2e",
};

export type Click = { at: number; x: number; y: number };
export type Caption = { at: number; lines: { text: string; accent?: string }[] };

export type Shot = {
	id: string;
	src?: string; // recording; none = a graphic shot (the end card)
	from?: number; // seconds into the recording
	to?: number;
	rate?: number; // playback speed
	freeze?: number; // seconds holding the last frame after `to`
	captions?: Caption[]; // `at` relative to the shot
	clicks?: Click[]; // `at` relative to the shot; x, y in the 1920x1080 frame
	linkGlow?: { at: number; x: number; y: number; w: number }; // ember underline on the agent's link
	timer?: { from: number; rate: number }; // a counter of real seconds, for the hunt
	sfx?: { at: number; src: string; volume?: number }[]; // sound effects, `at` relative to the shot
	length?: number; // graphic shots only
};

// Browser clicks come from record.mjs (time in its recording) and are shifted into each shot.
const browserClicks = (file: { clicks: { t: number; x: number; y: number }[] }, from: number, rate: number): Click[] =>
	file.clicks.map((c) => ({ at: (c.t - from) / rate, x: c.x, y: c.y })).filter((c) => c.at >= 0);

// Measured on the recordings (see the lessons log): where the path and the link sit in the reply.
export const PATH_AT = { x: 316, y: 610 };
export const LINK_AT = { x: 1390, y: 610, left: 1107, w: 570 };

export const shots: Shot[] = [
	{
		id: "hook",
		src: "out/scene12-path-hunt.mp4",
		from: 15.1,
		to: 18.9,
		freeze: 1.2,
		captions: [
			{ at: 0.2, lines: [{ text: "Your agent runs on a server." }] },
			{ at: 2.4, lines: [{ text: "Your agent runs on a server." }, { text: "Its docs stay there.", accent: "muted" }] },
		],
	},
	{
		id: "not-clickable",
		src: "out/scene12-path-hunt.mp4",
		from: 18.9,
		to: 18.9,
		freeze: 2.4,
		clicks: [
			{ at: 0.5, ...PATH_AT },
			{ at: 1.4, ...PATH_AT },
		],
		captions: [{ at: 0.6, lines: [{ text: "Not clickable." }] }],
	},
	{
		id: "hunt",
		src: "out/scene12-path-hunt.mp4",
		from: 19.6,
		to: 38.7,
		rate: 3,
		timer: { from: 0, rate: 3 },
		captions: [{ at: 0.3, lines: [{ text: "Open an editor. Dig. Squint." }] }],
	},
	{
		id: "link",
		src: "out/scene3-link.mp4",
		from: 15.1,
		to: 18.6,
		linkGlow: { at: 0.5, ...LINK_AT },
		clicks: [{ at: 2.3, x: LINK_AT.x, y: LINK_AT.y }],
		// The cut to the browser follows the click on the link.
		sfx: [{ at: 3.15, src: "audio/whoosh.wav", volume: 0.45 }],
		captions: [{ at: 0.3, lines: [{ text: "Same agent. " }, { text: "Now it hands you a link.", accent: "ember" }] }],
	},
	{
		id: "read",
		src: "out/scene4-read.mp4",
		from: 0.6,
		to: 9.0,
		clicks: browserClicks(edit4, 0.6, 1),
		captions: [{ at: 0.4, lines: [{ text: "One click. " }, { text: "Easy to read.", accent: "ember" }] }],
	},
	{
		id: "edit",
		src: "out/scene5-edit.mp4",
		from: 0.6,
		to: 10.2,
		rate: 1.1,
		clicks: browserClicks(edit5, 0.6, 1.1),
		captions: [
			{ at: 0.3, lines: [{ text: "Spot a mistake? " }, { text: "Fix it right here.", accent: "ember" }] },
			{ at: 5.6, lines: [{ text: "No IDE needed." }] },
		],
	},
	{
		id: "links",
		src: "out/scene6-link.mp4",
		from: 1.4,
		to: 7.2,
		rate: 1.6,
		clicks: browserClicks(edit6, 1.4, 1.6),
		captions: [{ at: 0.2, lines: [{ text: "Every link works." }] }],
	},
	{ id: "cta", length: 5 },
];

export const shotSeconds = (s: Shot) =>
	s.length ?? ((s.to ?? 0) - (s.from ?? 0)) / (s.rate ?? 1) + (s.freeze ?? 0);

export const totalSeconds = shots.reduce((sum, s) => sum + shotSeconds(s), 0);
