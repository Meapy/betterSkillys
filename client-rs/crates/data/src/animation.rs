//! `<Animation>` on objects (`AnimationsData.as`, `AnimationData.as`, `FrameData.as`): short
//! texture sequences played now and then, like a blinking eye or a flickering torch.

use crate::texture::TextureData;
use crate::xml::{to_int32, Element};

#[derive(Debug, Clone)]
pub struct AnimationData {
    /// The chance it plays when its time comes (1: always).
    pub prob: f64,
    /// Milliseconds between runs, give or take `period_jitter`.
    pub period: i32,
    pub period_jitter: i32,
    /// Runs on multiples of the period, so every object of the type plays together.
    pub sync: bool,
    pub frames: Vec<FrameData>,
}

#[derive(Debug, Clone)]
pub struct FrameData {
    /// How long the frame shows, milliseconds.
    pub time: i32,
    pub texture: TextureData,
}

impl AnimationData {
    pub fn from_xml(xml: &Element) -> Self {
        AnimationData {
            prob: xml.attr_number("prob", 1.0),
            period: to_int32(xml.attr_number("period", 0.0) * 1000.0),
            period_jitter: to_int32(xml.attr_number("periodJitter", 0.0) * 1000.0),
            sync: xml.attr("sync") == Some("true"),
            frames: xml
                .children_named("Frame")
                .map(|f| FrameData {
                    time: to_int32(f.attr_number("time", 0.0) * 1000.0),
                    texture: TextureData::from_xml(f),
                })
                .collect(),
        }
    }

    /// `getPeriod`; `random` is in [0, 1).
    fn period(&self, random: f64) -> i32 {
        if self.period_jitter == 0 {
            return self.period;
        }
        to_int32((self.period - self.period_jitter) as f64 + 2.0 * random * self.period_jitter as f64)
    }

    /// `getLastRun`: when it first plays.
    pub fn first_run(&self, time: i32, random: &mut impl FnMut() -> f64) -> i32 {
        if self.sync && self.period > 0 {
            return time / self.period * self.period;
        }
        time + self.period(random()) + to_int32(200.0 * random())
    }

    /// `getNextRun`.
    pub fn next_run(&self, time: i32, random: &mut impl FnMut() -> f64) -> i32 {
        if self.sync && self.period > 0 {
            return time / self.period * self.period + self.period;
        }
        time + self.period(random())
    }
}

/// One object's animation state (`Animations.as`).
#[derive(Debug, Clone, Default)]
pub struct Animations {
    next_run: Vec<i32>,
    /// (animation, frame, when the frame started).
    running: Option<(usize, usize, i32)>,
}

impl Animations {
    /// `getTexture`: which (animation, frame) shows at `time`, if one is playing.
    pub fn frame(&mut self, anims: &[AnimationData], time: i32, random: &mut impl FnMut() -> f64) -> Option<(usize, usize)> {
        if self.next_run.len() != anims.len() {
            self.next_run = anims.iter().map(|a| a.first_run(time, random)).collect();
        }
        if let Some((a, mut f, mut start)) = self.running {
            let frames = &anims[a].frames;
            loop {
                if time - start <= frames[f].time {
                    self.running = Some((a, f, start));
                    return Some((a, f));
                }
                if f + 1 >= frames.len() {
                    break;
                }
                start += frames[f].time;
                f += 1;
            }
            self.running = None;
        }
        for (i, anim) in anims.iter().enumerate() {
            if time > self.next_run[i] {
                let start = self.next_run[i];
                self.next_run[i] = anim.next_run(time, random);
                if anim.frames.is_empty() || (anim.prob != 1.0 && random() > anim.prob) {
                    continue;
                }
                self.running = Some((i, 0, start));
                return self.frame(anims, time, random);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim(sync: bool) -> AnimationData {
        let frame = |ms| FrameData { time: ms, texture: TextureData::from_xml(&Element::default()) };
        AnimationData { prob: 1.0, period: 1000, period_jitter: 0, sync, frames: vec![frame(100), frame(100)] }
    }

    #[test]
    fn synced_runs_on_the_period() {
        let anims = [anim(true)];
        let mut state = Animations::default();
        let mut r = || 0.5;
        // Starts at the last multiple of the period (1000), so at 1050 frame 0 shows, 1150 frame 1.
        assert_eq!(state.frame(&anims, 1050, &mut r), Some((0, 0)));
        assert_eq!(state.frame(&anims, 1060, &mut r), Some((0, 0)));
        assert_eq!(state.frame(&anims, 1150, &mut r), Some((0, 1)));
        assert_eq!(state.frame(&anims, 1250, &mut r), None);
        assert_eq!(state.frame(&anims, 2010, &mut r), Some((0, 0)));
    }
}
