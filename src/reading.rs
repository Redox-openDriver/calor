use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Place {
    Processor,
    Graphics,
    Board,
    Storage,
    Wireless,
    Battery,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sensor {
    pub place: Place,
    pub label: String,
    pub millis: i32,
    pub critical: Option<i32>,
}

impl Sensor {
    pub fn celsius(&self) -> f32 {
        self.millis as f32 / 1000.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reading {
    pub time: u64,
    pub sensors: Vec<Sensor>,
    pub power: Option<u32>,
    pub fan: Option<bool>,
}

impl Reading {
    pub fn processor(&self) -> Option<&Sensor> {
        self.sensors.iter().filter(|sensor| sensor.place == Place::Processor).max_by_key(|sensor| sensor.millis)
    }

    pub fn reference(&self) -> Option<&Sensor> {
        self.sensors.iter().filter(|sensor| sensor.place != Place::Processor && sensor.place != Place::Graphics).min_by_key(|sensor| sensor.millis)
    }

    pub fn hottest(&self) -> Option<&Sensor> {
        self.sensors.iter().max_by_key(|sensor| sensor.millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    pub fn sensor(place: Place, label: &str, celsius: i32) -> Sensor {
        Sensor { place, label: label.to_string(), millis: celsius * 1000, critical: None }
    }

    #[test]
    fn the_processor_the_reference_and_the_hottest_are_found() {
        let reading = Reading {
            time: 0,
            sensors: vec![
                sensor(Place::Processor, "Tctl", 61),
                sensor(Place::Graphics, "edge", 64),
                sensor(Place::Board, "acpitz", 45),
                sensor(Place::Storage, "nvme", 38),
                sensor(Place::Wireless, "mt7921", 47),
            ],
            power: Some(12_000),
            fan: Some(true),
        };
        assert_eq!(reading.processor().unwrap().label, "Tctl");
        assert_eq!(reading.reference().unwrap().label, "nvme");
        assert_eq!(reading.hottest().unwrap().label, "edge");
        assert_eq!(reading.processor().unwrap().celsius(), 61.0);
        assert!(Reading::default().processor().is_none());
    }
}
