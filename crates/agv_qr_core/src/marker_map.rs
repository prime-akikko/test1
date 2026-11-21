//! Marker map store: `id → (x, y, yaw, size)` (design §3).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// One surveyed floor fiducial.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub id: u32,
    /// Metres, map frame (marker center).
    pub x_m: f64,
    /// Metres, map frame.
    pub y_m: f64,
    /// Radians, printed “up” axis yaw in map frame.
    pub yaw_rad: f64,
    /// Outer square side length in metres.
    pub size_m: f64,
}

impl Marker {
    pub fn new(id: u32, x_m: f64, y_m: f64, yaw_rad: f64, size_m: f64) -> Self {
        Self {
            id,
            x_m,
            y_m,
            yaw_rad,
            size_m,
        }
    }
}

/// Static site marker map (JSON/YAML file on the AGV in production).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MarkerMap {
    pub version: String,
    pub markers: BTreeMap<u32, Marker>,
}

impl MarkerMap {
    /// Lab default: 3×3 grid at 1 m centres, 0.1 m markers (design milestone 3).
    pub fn lab_grid_3x3() -> Self {
        let mut markers = BTreeMap::new();
        let size_m = 0.10;
        let mut id = 1u32;
        for row in 0..3 {
            for col in 0..3 {
                let x = col as f64 * 1.0;
                let y = row as f64 * 1.0;
                markers.insert(id, Marker::new(id, x, y, 0.0, size_m));
                id += 1;
            }
        }
        Self {
            version: "lab-3x3-v1".into(),
            markers,
        }
    }

    pub fn get(&self, id: u32) -> Option<&Marker> {
        self.markers.get(&id)
    }

    pub fn len(&self) -> usize {
        self.markers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.markers.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u32, &Marker)> {
        self.markers.iter()
    }

    /// Nearest marker to a planar point (for sim “detection”).
    pub fn nearest(&self, x_m: f64, y_m: f64) -> Option<&Marker> {
        self.markers.values().min_by(|a, b| {
            let da = (a.x_m - x_m).hypot(a.y_m - y_m);
            let db = (b.x_m - x_m).hypot(b.y_m - y_m);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn load_path(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let raw = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&raw)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_grid_has_nine_markers() {
        let map = MarkerMap::lab_grid_3x3();
        assert_eq!(map.len(), 9);
        assert_eq!(map.version, "lab-3x3-v1");
        let m = map.get(5).unwrap();
        assert!((m.x_m - 1.0).abs() < 1e-9);
        assert!((m.y_m - 1.0).abs() < 1e-9);
    }

    #[test]
    fn nearest_at_origin_is_id_1() {
        let map = MarkerMap::lab_grid_3x3();
        let n = map.nearest(0.05, 0.05).unwrap();
        assert_eq!(n.id, 1);
    }

    #[test]
    fn json_roundtrip() {
        let map = MarkerMap::lab_grid_3x3();
        let json = map.to_json_pretty().unwrap();
        let back = MarkerMap::from_json(&json).unwrap();
        assert_eq!(map, back);
    }
}
