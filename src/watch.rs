use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use crate::cooling::Change;
use crate::reading::Reading;

const REARM: i32 = 5_000;
const WORSE_STEP: f32 = 5.0;
pub const WORSE: f32 = 15.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub hot: i32,
    pub hold: u64,
    pub margin: i32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { hot: 90_000, hold: 30_000_000, margin: 5_000 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Hot { label: String, celsius: i32 },
    Cooled { label: String, celsius: i32 },
    NearCritical { label: String, celsius: i32, critical: i32 },
    CoolingWorse { percent: u32 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Watch {
    limits: Limits,
    hot_since: Option<u64>,
    hot: bool,
    near: BTreeSet<String>,
    worse: Option<u32>,
}

impl Watch {
    pub fn new(limits: Limits) -> Watch {
        Watch { limits, ..Watch::default() }
    }

    pub fn push(&mut self, reading: &Reading) -> Vec<Notice> {
        let mut out = Vec::new();
        if let Some(processor) = reading.processor() {
            if processor.millis >= self.limits.hot {
                let since = *self.hot_since.get_or_insert(reading.time);
                if !self.hot && reading.time.saturating_sub(since) >= self.limits.hold {
                    self.hot = true;
                    out.push(Notice::Hot { label: processor.label.clone(), celsius: processor.millis / 1000 });
                }
            } else {
                self.hot_since = None;
                if self.hot && processor.millis < self.limits.hot - REARM {
                    self.hot = false;
                    out.push(Notice::Cooled { label: processor.label.clone(), celsius: processor.millis / 1000 });
                }
            }
        }
        for sensor in &reading.sensors {
            let Some(critical) = sensor.critical else { continue };
            if sensor.millis >= critical - self.limits.margin {
                if self.near.insert(sensor.label.clone()) {
                    out.push(Notice::NearCritical { label: sensor.label.clone(), celsius: sensor.millis / 1000, critical: critical / 1000 });
                }
            } else if sensor.millis < critical - self.limits.margin - REARM {
                self.near.remove(&sensor.label);
            }
        }
        out
    }

    pub fn cooling(&mut self, change: Option<Change>) -> Option<Notice> {
        let change = change.filter(|change| change.percent >= WORSE)?;
        let step = (change.percent / WORSE_STEP) as u32;
        if self.worse.is_some_and(|told| told >= step) {
            return None;
        }
        self.worse = Some(step);
        Some(Notice::CoolingWorse { percent: change.percent as u32 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reading::{Place, Sensor};
    use alloc::string::ToString;
    use alloc::vec;

    fn reading(time: u64, processor: i32, board: i32) -> Reading {
        Reading {
            time,
            sensors: vec![
                Sensor { place: Place::Processor, label: "Tctl".to_string(), millis: processor, critical: None },
                Sensor { place: Place::Board, label: "THS0".to_string(), millis: board, critical: Some(120_000) },
            ],
            power: None,
            fan: None,
        }
    }

    #[test]
    fn heat_is_told_once_it_lasts_and_again_once_it_is_gone() {
        let mut watch = Watch::new(Limits::default());
        assert!(watch.push(&reading(0, 95_000, 50_000)).is_empty());
        assert!(watch.push(&reading(20_000_000, 95_000, 50_000)).is_empty());
        assert_eq!(watch.push(&reading(30_000_000, 96_000, 50_000)), [Notice::Hot { label: "Tctl".to_string(), celsius: 96 }]);
        assert!(watch.push(&reading(40_000_000, 97_000, 50_000)).is_empty());
        assert!(watch.push(&reading(50_000_000, 88_000, 50_000)).is_empty(), "not cooled until clearly below");
        assert_eq!(watch.push(&reading(60_000_000, 80_000, 50_000)), [Notice::Cooled { label: "Tctl".to_string(), celsius: 80 }]);
        assert!(watch.push(&reading(70_000_000, 95_000, 50_000)).is_empty(), "a short spike is not heat");
    }

    #[test]
    fn a_sensor_near_its_critical_limit_is_told_once() {
        let mut watch = Watch::new(Limits::default());
        assert_eq!(watch.push(&reading(0, 60_000, 116_000)), [Notice::NearCritical { label: "THS0".to_string(), celsius: 116, critical: 120 }]);
        assert!(watch.push(&reading(1, 60_000, 117_000)).is_empty());
        assert!(watch.push(&reading(2, 60_000, 112_000)).is_empty());
        assert!(watch.push(&reading(3, 60_000, 109_000)).is_empty());
        assert_eq!(watch.push(&reading(4, 60_000, 115_000)).len(), 1);
    }

    #[test]
    fn worse_cooling_is_told_at_each_new_step_only() {
        let mut watch = Watch::default();
        assert_eq!(watch.cooling(Some(Change { percent: 10.0, since: 0 })), None);
        assert_eq!(watch.cooling(Some(Change { percent: 17.0, since: 0 })), Some(Notice::CoolingWorse { percent: 17 }));
        assert_eq!(watch.cooling(Some(Change { percent: 18.0, since: 0 })), None);
        assert_eq!(watch.cooling(Some(Change { percent: 21.0, since: 0 })), Some(Notice::CoolingWorse { percent: 21 }));
        assert_eq!(watch.cooling(None), None);
    }
}
