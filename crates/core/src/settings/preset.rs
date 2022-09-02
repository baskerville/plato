use chrono::{Local, Timelike};
use serde::{Serialize, Deserialize};
use crate::frontlight::LightLevels;
use crate::geom::circular_distances;

const MINUTES_PER_DAY: u16 = 24 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LightPreset {
    pub timestamp: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lightsensor_level: Option<u16>,
    pub frontlight_levels: LightLevels,
}

impl Default for LightPreset {
    fn default() -> Self {
        let now = Local::now();
        LightPreset {
            timestamp: (60 * now.hour() + now.minute()) as u16,
            frontlight_levels: LightLevels::default(),
            lightsensor_level: None,
        }
    }
}

impl LightPreset {
    pub fn name(&self) -> String {
        let hours = self.timestamp / 60;
        let minutes = self.timestamp - hours * 60;
        format!("{:02}:{:02}", hours, minutes)
    }
}

pub fn guess_frontlight(lightsensor_level: Option<u16>, light_presets: &[LightPreset]) -> Option<LightLevels> {
    if light_presets.len() < 2 {
        return None;
    }
    let cur = LightPreset {
        lightsensor_level,
        .. Default::default()
    };

    let mut dmin = [u16::MAX; 2];
    let mut index = [usize::MAX; 2];

    if light_presets[0].lightsensor_level.is_some() {
        let s = cur.lightsensor_level.unwrap_or_default();

        for (i, lp) in light_presets.iter().enumerate() {
            let p = lp.lightsensor_level.unwrap_or_default();
            let d = if s >= p { s - p } else { p - s };

            if p >= s && d < dmin[0] {
                dmin[0] = d;
                index[0] = i;
            }

            if p <= s && d < dmin[1] {
                dmin[1] = d;
                index[1] = i;
            }
        }
    } else {
        for (i, lp) in light_presets.iter().enumerate() {
            let (d0, d1) = circular_distances(cur.timestamp, lp.timestamp, MINUTES_PER_DAY);

            if d0 < dmin[0] {
                dmin[0] = d0;
                index[0] = i;
            }

            if d1 < dmin[1] {
                dmin[1] = d1;
                index[1] = i;
            }
        }
    }

    if dmin[0] == 0 || dmin[1] == u16::MAX {
        return Some(light_presets[index[0]].frontlight_levels);
    }
    
    if dmin[1] == 0 || dmin[0] == u16::MAX {
        return Some(light_presets[index[1]].frontlight_levels);
    }

    let fl0 = light_presets[index[0]].frontlight_levels;
    let fl1 = light_presets[index[1]].frontlight_levels;
    let t = dmin[0] as f32 / (dmin[0] + dmin[1]) as f32;

    Some(fl0.interpolate(fl1, t))
}
