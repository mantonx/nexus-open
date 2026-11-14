//! Fixed sample data for previews, examples and image tests.
//!
//! Not real readings. Lets the renderer be exercised the same way everywhere
//! until live signals exist.

use crate::widget::{Graph, Severity, WidgetView, Zone};

/// The System page from the current app, with representative data.
pub fn system_page() -> Vec<Zone> {
    let flat: Vec<f32> = (0..60)
        .map(|i| 0.12 + 0.03 * ((i as f32) * 0.7).sin())
        .collect();
    let spiky: Vec<f32> = (0..60)
        .map(|i| match i {
            6 => 0.85,
            7 => 0.4,
            8 => 0.2,
            _ => 0.08 + 0.02 * ((i as f32) * 1.3).sin(),
        })
        .collect();
    vec![
        Zone {
            width: 128,
            view: WidgetView::Clock {
                date: "Tue, Jun 09".into(),
                time: "4:17".into(),
                meridiem: Some("PM".into()),
            },
        },
        Zone {
            width: 128,
            view: WidgetView::Value {
                label: "Jersey City".into(),
                value: "79".into(),
                unit: "°F".into(),
                icon: Some('\u{f185}'),
                graph: None,
                severity: Severity::Ok,
            },
        },
        Zone {
            width: 128,
            view: WidgetView::Value {
                label: "CPU".into(),
                value: "28".into(),
                unit: "°C".into(),
                icon: None,
                graph: Some(Graph::Area(flat.clone())),
                severity: Severity::Ok,
            },
        },
        Zone {
            width: 128,
            view: WidgetView::Value {
                label: "GPU".into(),
                value: "82".into(),
                unit: "°C".into(),
                icon: None,
                graph: Some(Graph::Area(flat)),
                severity: Severity::Warn,
            },
        },
        Zone {
            width: 128,
            view: WidgetView::Rate {
                label: "Net".into(),
                caption: "↓292K/s ↑309K/s".into(),
                graph: Some(Graph::Line(spiky)),
            },
        },
    ]
}
