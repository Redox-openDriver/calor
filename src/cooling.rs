use alloc::vec::Vec;

const STEADY: u64 = 120_000_000;
const STEADY_SPREAD: f32 = 1.5;
const POWER_SPREAD: f32 = 0.15;
const BUCKETS: [u32; 5] = [3_000, 6_000, 10_000, 15_000, 25_000];
const SAMPLE_GAP: u64 = 600_000_000;
const BASELINE_DAYS: u32 = 14;
const RECENT_DAYS: u32 = 7;
const LEAST: usize = 5;
const SMALLEST_RISE: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub day: u32,
    pub bucket: u8,
    pub rise: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Change {
    pub percent: f32,
    pub since: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cooling {
    window: Vec<(u64, f32, f32)>,
    sampled: [Option<u64>; BUCKETS.len()],
}

impl Cooling {
    pub fn observe(&mut self, time: u64, day: u32, processor: f32, reference: f32, power: u32) -> Option<Sample> {
        let watts = power as f32 / 1000.0;
        self.window.retain(|(at, _, _)| time >= *at && time - *at <= STEADY);
        self.window.push((time, processor, watts));
        let first = self.window.first()?.0;
        if time - first < STEADY * 9 / 10 {
            return None;
        }
        let (coolest, warmest) = self.window.iter().fold((f32::MAX, f32::MIN), |(low, high), (_, celsius, _)| (low.min(*celsius), high.max(*celsius)));
        let mean = self.window.iter().map(|(_, _, watts)| watts).sum::<f32>() / self.window.len() as f32;
        let steady_power = mean > 0.0 && self.window.iter().all(|(_, _, watts)| (watts - mean).abs() <= mean * POWER_SPREAD);
        if warmest - coolest > STEADY_SPREAD || !steady_power {
            return None;
        }
        let bucket = BUCKETS.iter().rposition(|low| power >= *low)?;
        if self.sampled[bucket].is_some_and(|at| time.saturating_sub(at) < SAMPLE_GAP) {
            return None;
        }
        self.sampled[bucket] = Some(time);
        Some(Sample { day, bucket: bucket as u8, rise: processor - reference })
    }
}

fn median(mut values: Vec<f32>) -> f32 {
    values.sort_by(f32::total_cmp);
    values[values.len() / 2]
}

pub fn assess(samples: &[Sample], today: u32) -> Option<Change> {
    let since = samples.iter().map(|sample| sample.day).min()?;
    let (mut weighted, mut weights) = (0.0f32, 0.0f32);
    for bucket in 0..BUCKETS.len() as u8 {
        let base: Vec<f32> = samples.iter().filter(|sample| sample.bucket == bucket && sample.day < since + BASELINE_DAYS).map(|sample| sample.rise).collect();
        let recent: Vec<f32> = samples
            .iter()
            .filter(|sample| sample.bucket == bucket && sample.day >= since + BASELINE_DAYS && sample.day + RECENT_DAYS > today && sample.day <= today)
            .map(|sample| sample.rise)
            .collect();
        if base.len() < LEAST || recent.len() < LEAST {
            continue;
        }
        let weight = base.len().min(recent.len()) as f32;
        let (base, recent) = (median(base), median(recent));
        if base < SMALLEST_RISE {
            continue;
        }
        weighted += (recent - base) / base * 100.0 * weight;
        weights += weight;
    }
    (weights > 0.0).then(|| Change { percent: weighted / weights, since })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    const STEP: u64 = 10_000_000;

    fn run(cooling: &mut Cooling, from: u64, steps: u64, processor: impl Fn(u64) -> f32, power: u32) -> Vec<Sample> {
        (0..steps).filter_map(|step| cooling.observe(from + step * STEP, 20_000, processor(step), 35.0, power)).collect()
    }

    #[test]
    fn only_a_steady_load_gives_a_sample_and_not_too_often() {
        let mut cooling = Cooling::default();
        let samples = run(&mut cooling, 0, 30, |_| 60.0, 12_000);
        assert_eq!(samples, [Sample { day: 20_000, bucket: 2, rise: 25.0 }], "one sample per bucket in ten minutes");
        let mut rising = Cooling::default();
        assert!(run(&mut rising, 0, 30, |step| 50.0 + step as f32 * 0.5, 12_000).is_empty());
        let mut busy = Cooling::default();
        assert!((0..30).filter_map(|step| busy.observe(step * STEP, 1, 60.0, 35.0, if step % 2 == 0 { 8_000 } else { 16_000 })).next().is_none());
        let later = run(&mut cooling, 700_000_000, 30, |_| 61.0, 12_000);
        assert_eq!(later.len(), 1);
        assert!(run(&mut Cooling::default(), 0, 30, |_| 40.0, 1_000).is_empty(), "idle under the lowest bucket is not measured");
    }

    fn samples(days: core::ops::Range<u32>, rise: f32) -> Vec<Sample> {
        days.flat_map(|day| vec![Sample { day, bucket: 2, rise }, Sample { day, bucket: 3, rise: rise + 10.0 }]).collect()
    }

    #[test]
    fn cooling_that_got_worse_week_on_week_is_measured() {
        let mut all = samples(0..14, 20.0);
        all.extend(samples(30..37, 25.0));
        let change = assess(&all, 36).unwrap();
        assert!((change.percent - 20.83).abs() < 0.01, "{change:?}");
        assert_eq!(change.since, 0);
        assert!(assess(&samples(0..14, 20.0), 13).is_none(), "nothing recent to compare yet");
        let mut few = samples(0..3, 20.0);
        few.extend(samples(30..37, 25.0));
        assert!(assess(&few, 36).is_none(), "too few days for a baseline");
        let mut same = samples(0..14, 20.0);
        same.extend(samples(30..37, 20.0));
        assert_eq!(assess(&same, 36).unwrap().percent, 0.0);
        assert!(assess(&[], 0).is_none());
    }
}
