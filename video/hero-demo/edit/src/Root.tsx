import { Composition } from "remotion";
import { FPS, frames, type Shot, shots, shotSeconds } from "./data";
import { Hero } from "./Hero";

const length = (cut: Shot[]) => cut.reduce((sum, s) => sum + frames(shotSeconds(s)), 0);

// The README cut (about 30 s): the same story, tighter.
const tighter: Record<string, Partial<Shot>> = {
	hook: { from: 15.4, to: 18.9, freeze: 0.1 },
	"not-clickable": { freeze: 1.8 },
	hunt: { rate: 5, timer: { from: 0, rate: 5 } },
	read: { to: 6.8 },
	edit: { rate: 1.5 },
	links: { to: 6.2, rate: 2 },
	cta: { length: 4 },
};
const readmeCut: Shot[] = shots.map((s) => ({ ...s, ...(tighter[s.id] ?? {}) }));

export const Root = () => (
	<>
		<Composition id="Hero" component={Hero} durationInFrames={length(shots)} fps={FPS} width={1920} height={1080} defaultProps={{ cut: shots }} />
		<Composition id="HeroReadme" component={Hero} durationInFrames={length(readmeCut)} fps={FPS} width={1920} height={1080} defaultProps={{ cut: readmeCut }} />
	</>
);
