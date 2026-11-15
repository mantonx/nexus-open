//! Where widgets sit on the strip, and what a drag does to them. Plain
//! functions over widths in strip pixels, so the rules are tested here and
//! the window only draws the result.
//!
//! Widgets sit edge to edge from x = 0; whatever is left over is empty
//! strip on the right.

use nexus_core::config::MIN_WIDGET_WIDTH;

/// Strip width in pixels.
pub const STRIP: u32 = 640;

/// Each widget's left edge.
pub fn lefts(widths: &[u32]) -> Vec<u32> {
    widths
        .iter()
        .scan(0, |x, w| {
            let left = *x;
            *x += w;
            Some(left)
        })
        .collect()
}

/// The widths after dragging the right edge of widget `edge` by `delta`
/// pixels (positive is rightwards).
///
/// Between two widgets the edge trades width between them, so everything
/// else stays put. The last widget's right edge grows into the empty strip
/// or gives width back to it. No widget gets narrower than the minimum;
/// a drag past that stops at it.
pub fn resize(widths: &[u32], edge: usize, delta: i32) -> Vec<u32> {
    let mut out = widths.to_vec();
    let Some(&w) = widths.get(edge) else {
        return out;
    };
    let min = MIN_WIDGET_WIDTH as i32;
    let w = w as i32;
    let (shrink, grow) = match widths.get(edge + 1) {
        Some(&next) => (w - min, next as i32 - min),
        None => (w - min, (STRIP - widths.iter().sum::<u32>()) as i32),
    };
    let d = delta.clamp(-shrink.max(0), grow.max(0));
    out[edge] = (w + d) as u32;
    if let Some(next) = out.get_mut(edge + 1) {
        *next = (*next as i32 - d) as u32;
    }
    out
}

/// Where widget `index` lands if dropped with its left edge at `left`
/// (strip pixels). It passes a neighbour once its leading edge crosses
/// that neighbour's middle: its right edge going right, its left edge
/// going left. Half a neighbour's width of travel, as sortable lists do.
pub fn drop_index(widths: &[u32], index: usize, left: f32) -> usize {
    let Some(&own) = widths.get(index) else {
        return index;
    };
    let right = left + own as f32;
    lefts(widths)
        .iter()
        .zip(widths)
        .enumerate()
        .filter(|&(i, (&x, &w))| {
            let middle = x as f32 + w as f32 / 2.0;
            match i.cmp(&index) {
                std::cmp::Ordering::Less => middle <= left,
                std::cmp::Ordering::Greater => middle < right,
                std::cmp::Ordering::Equal => false,
            }
        })
        .count()
}

/// The x of the gap a widget would drop into: the left edge of the widget
/// at `to` once `index` is taken out of the row.
pub fn drop_x(widths: &[u32], index: usize, to: usize) -> u32 {
    widths
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != index)
        .take(to)
        .map(|(_, w)| w)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lefts_run_edge_to_edge() {
        assert_eq!(lefts(&[100, 200, 50]), vec![0, 100, 300]);
        assert_eq!(lefts(&[]), Vec::<u32>::new());
    }

    #[test]
    fn an_inner_edge_trades_width_with_the_neighbour() {
        assert_eq!(resize(&[100, 200, 340], 0, 30), vec![130, 170, 340]);
        assert_eq!(resize(&[100, 200, 340], 1, -50), vec![100, 150, 390]);
    }

    #[test]
    fn no_widget_gets_narrower_than_the_minimum() {
        let min = MIN_WIDGET_WIDTH;
        assert_eq!(resize(&[100, 200], 0, -500), vec![min, 260]);
        assert_eq!(resize(&[100, 200], 0, 500), vec![300 - min, min]);
    }

    #[test]
    fn the_last_edge_uses_the_empty_strip() {
        assert_eq!(resize(&[100, 200], 1, 1000), vec![100, 540]);
        assert_eq!(resize(&[100, 200], 1, -60), vec![100, 140]);
        // A full strip has nothing to grow into.
        assert_eq!(resize(&[320, 320], 1, 10), vec![320, 320]);
    }

    #[test]
    fn a_bad_edge_changes_nothing() {
        assert_eq!(resize(&[100], 3, 10), vec![100]);
    }

    #[test]
    fn a_widget_passes_a_neighbour_at_its_middle() {
        let w = [100, 100, 100, 100]; // middles 50, 150, 250, 350
        assert_eq!(drop_index(&w, 0, 0.0), 0); // not moved
        assert_eq!(drop_index(&w, 0, 40.0), 0); // right edge at 140
        assert_eq!(drop_index(&w, 0, 60.0), 1); // right edge past 150
        assert_eq!(drop_index(&w, 0, 999.0), 3); // to the end
        assert_eq!(drop_index(&w, 2, 160.0), 2); // left edge not past 150
        assert_eq!(drop_index(&w, 2, 140.0), 1); // left edge past 150
        assert_eq!(drop_index(&w, 3, -10.0), 0); // to the front
    }

    #[test]
    fn a_wide_widget_passes_a_narrow_one_quickly() {
        let w = [300, 50]; // the narrow one's middle is at 325
        assert_eq!(drop_index(&w, 0, 20.0), 0);
        assert_eq!(drop_index(&w, 0, 30.0), 1);
    }

    #[test]
    fn drop_gaps_skip_the_dragged_widget() {
        let w = [100, 50, 200];
        assert_eq!(drop_x(&w, 0, 0), 0);
        assert_eq!(drop_x(&w, 0, 1), 50);
        assert_eq!(drop_x(&w, 0, 2), 250);
        assert_eq!(drop_x(&w, 2, 1), 100);
    }
}
