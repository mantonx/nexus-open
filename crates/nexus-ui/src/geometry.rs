//! Where widgets sit on the strip, and what a drag does to them. Plain
//! functions over widths in strip pixels, so the rules are tested here and
//! the window only draws the result.
//!
//! Widgets sit edge to edge from x = 0; whatever is left over is empty
//! strip on the right.

use nexus_core::config::{MIN_WIDGET_WIDTH, Widget};

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

/// The narrowest and widest widget `index` can be made by moving its right
/// edge (as `resize` does): down to the minimum, up to what the widget on
/// its right, or the empty strip, can give.
pub fn width_bounds(widths: &[u32], index: usize) -> (u32, u32) {
    let Some(&w) = widths.get(index) else {
        return (MIN_WIDGET_WIDTH, STRIP);
    };
    let grow = match widths.get(index + 1) {
        Some(&next) => next.saturating_sub(MIN_WIDGET_WIDTH),
        None => STRIP.saturating_sub(widths.iter().sum()),
    };
    (MIN_WIDGET_WIDTH.min(w), w + grow)
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

/// Room for a new widget that would like `want` pixels: the widths the
/// others take, and the new one's width. Empty strip is used first; past
/// that every widget gives up width in proportion to what it has above the
/// minimum, so a wide widget gives more than a narrow one and none goes
/// below the minimum. The new one gets less than it wants only when the
/// others are all at the minimum. `None` when not even a minimum-width
/// widget fits.
#[cfg(test)]
pub fn make_room(widths: &[u32], want: u32) -> Option<(Vec<u32>, u32)> {
    let mins = vec![MIN_WIDGET_WIDTH; widths.len()];
    make_room_above(widths, &mins, want, MIN_WIDGET_WIDTH)
}

/// `make_room`, with a floor of its own for each widget (`mins`, where
/// it still reads) and for the new one (`new_min`).
pub fn make_room_above(
    widths: &[u32],
    mins: &[u32],
    want: u32,
    new_min: u32,
) -> Option<(Vec<u32>, u32)> {
    let new_min = new_min.max(MIN_WIDGET_WIDTH);
    let want = want.clamp(new_min, STRIP);
    let free = STRIP.saturating_sub(widths.iter().sum());
    if free >= want {
        return Some((widths.to_vec(), want));
    }
    let spare: Vec<u32> = widths
        .iter()
        .zip(mins)
        .map(|(w, m)| w.saturating_sub((*m).max(MIN_WIDGET_WIDTH)))
        .collect();
    let total: u32 = spare.iter().sum();
    if free + total < new_min {
        return None;
    }
    let take = (want - free).min(total);
    // Largest remainder: each gives its share rounded down, then the
    // pixels left go to the ones that lost most to rounding.
    let mut given: Vec<u32> = spare
        .iter()
        .map(|&s| (u64::from(s) * u64::from(take) / u64::from(total.max(1))) as u32)
        .collect();
    let mut short = take - given.iter().sum::<u32>();
    let mut order: Vec<usize> = (0..spare.len()).collect();
    order.sort_by_key(|&i| {
        let exact = u64::from(spare[i]) * u64::from(take) % u64::from(total.max(1));
        std::cmp::Reverse(exact)
    });
    for i in order.into_iter().cycle().take(spare.len() * 2) {
        if short == 0 {
            break;
        }
        if given[i] < spare[i] {
            given[i] += 1;
            short -= 1;
        }
    }
    let out = widths.iter().zip(&given).map(|(w, g)| w - g).collect();
    Some((out, free + take))
}

/// The narrowest a widget of this kind still reads at: a key shows its
/// icon at the minimum, a reading needs room for its number, music for
/// its title, news its headline. Add never takes a widget below this.
pub fn readable(w: &Widget) -> u32 {
    match w {
        Widget::Key { .. } => MIN_WIDGET_WIDTH,
        Widget::Value { .. } | Widget::Rate { .. } | Widget::Clock { .. } => 72,
        Widget::Slider { .. } => 80,
        // A headline, like a title, needs room for some of its words.
        Widget::Media { .. } | Widget::News { .. } => 120,
    }
}

/// Whether anything more fits on a row with these widths.
#[cfg(test)]
pub fn has_room(widths: &[u32]) -> bool {
    make_room(widths, MIN_WIDGET_WIDTH).is_some()
}

/// `n` widgets sharing the whole strip equally; the pixels that do not
/// divide go one each to the first ones.
pub fn equal(n: usize) -> Vec<u32> {
    if n == 0 {
        return Vec::new();
    }
    let n32 = n as u32;
    (0..n32)
        .map(|i| STRIP / n32 + u32::from(i < STRIP % n32))
        .collect()
}

/// How near, in strip pixels, a dragged edge must come to a snap point to
/// land on it.
pub const SNAP: i32 = 5;
/// Widths where a widget changes what it shows (`DESIGN.md`, size
/// classes): medium from 150, hero from 300.
pub const CLASS_STEPS: [u32; 2] = [150, 300];

/// `delta` for dragging the right edge of `edge`, pulled onto the nearest
/// snap point within `SNAP`: the two neighbours equal, either of them at a
/// size-class step, or a third, quarter or half of the strip. Returns the
/// delta to use and whether it snapped.
pub fn snap(widths: &[u32], edge: usize, delta: i32) -> (i32, bool) {
    let Some(&w) = widths.get(edge) else {
        return (delta, false);
    };
    let left = lefts(widths)[edge] as i32;
    let right = left + w as i32;
    let x = right + delta;
    let mut points: Vec<i32> = [160, 213, 320, 427, 480].to_vec();
    for step in CLASS_STEPS {
        points.push(left + step as i32);
    }
    if let Some(&next) = widths.get(edge + 1) {
        let end = right + next as i32;
        points.push(left + (w + next) as i32 / 2);
        for step in CLASS_STEPS {
            points.push(end - step as i32);
        }
    }
    let best = points
        .into_iter()
        .filter(|p| (p - x).abs() <= SNAP)
        .min_by_key(|p| (p - x).abs());
    match best {
        Some(p) => (p - right, true),
        None => (delta, false),
    }
}

/// The size class a width falls in, in the window's words.
pub fn class(width: u32) -> &'static str {
    if width >= CLASS_STEPS[1] {
        "Large"
    } else if width >= CLASS_STEPS[0] {
        "Medium"
    } else {
        "Small"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The width field shows the range a resize can reach.
    #[test]
    fn width_bounds_are_what_a_resize_can_reach() {
        assert_eq!(width_bounds(&[200, 300, 100], 0), (40, 460));
        assert_eq!(width_bounds(&[200, 300, 100], 2), (40, 140), "into the free 40");
        assert_eq!(width_bounds(&[320, 320], 1), (40, 320), "nothing free at the end");
        assert_eq!(width_bounds(&[30, 100], 0), (30, 90), "already under the minimum");
        let (_, max) = width_bounds(&[200, 300, 100], 0);
        assert_eq!(resize(&[200, 300, 100], 0, 1000)[0], max);
    }

    /// Add takes room only down to where each widget still reads.
    #[test]
    fn room_is_made_only_down_to_where_each_reads() {
        // A clock at 568 and a media widget at 72 already narrow.
        let (out, w) = make_room_above(&[568, 72], &[72, 120], 72, 40).unwrap();
        assert_eq!(w, 72);
        assert_eq!(out, vec![496, 72], "media below its floor gives nothing");
        // Nothing above the floors: no room, even though 40 px would fit.
        assert_eq!(make_room_above(&[320, 320], &[320, 320], 72, 40), None);
        assert_eq!(make_room(&[320, 320], 72).map(|r| r.1), Some(72));
    }

    #[test]
    fn room_comes_from_the_empty_strip_first() {
        assert_eq!(make_room(&[200, 200], 120), Some((vec![200, 200], 120)));
        // 40 free, 80 more taken from the two in proportion to their spare.
        let (others, new) = make_room(&[300, 300], 120).unwrap();
        assert_eq!(new, 120);
        assert_eq!(others, vec![260, 260]);
        assert_eq!(others.iter().sum::<u32>() + new, STRIP);
    }

    #[test]
    fn a_wide_widget_gives_more_than_a_narrow_one() {
        let (others, new) = make_room(&[440, 200], 160).unwrap();
        assert_eq!(new, 160);
        // Spare 400 and 160: they give 114 and 46.
        assert_eq!(others, vec![326, 154]);
        assert_eq!(others.iter().sum::<u32>() + new, STRIP);
    }

    #[test]
    fn nobody_goes_below_the_minimum() {
        let min = MIN_WIDGET_WIDTH;
        let widths = [min, min, min, 640 - 3 * min];
        let (others, new) = make_room(&widths, 200).unwrap();
        assert_eq!(new, 200);
        assert!(others.iter().all(|&w| w >= min), "{others:?}");
        assert_eq!(&others[..3], &[min, min, min]);
    }

    #[test]
    fn a_crowded_row_gives_what_it_can_and_a_full_one_nothing() {
        let min = MIN_WIDGET_WIDTH;
        // 14 at the minimum, one with 40 spare: the new one gets those 40.
        let mut widths = vec![min; 14];
        widths.push(640 - 14 * min);
        let (others, new) = make_room(&widths, 200).unwrap();
        assert_eq!(new, 40);
        assert!(others.iter().all(|&w| w == min));
        // Every widget at the minimum and the strip full: no room.
        assert_eq!(make_room(&[min; 16], 100), None);
        assert!(!has_room(&[min; 16]));
        assert!(has_room(&[]));
    }

    #[test]
    fn equal_widths_fill_the_strip() {
        assert_eq!(equal(4), vec![160; 4]);
        assert_eq!(equal(3), vec![214, 213, 213]);
        assert_eq!(equal(3).iter().sum::<u32>(), STRIP);
        assert!(equal(0).is_empty());
    }

    #[test]
    fn an_edge_snaps_to_equal_neighbours_and_class_steps() {
        // Two widgets 100 and 300: equal at x = 200, 98 px of travel.
        assert_eq!(snap(&[100, 300], 0, 97), (100, true));
        // A widget starting at 0 reaches the medium step at 150.
        assert_eq!(snap(&[100, 300], 0, 48), (50, true));
        // Nowhere near anything: unchanged.
        assert_eq!(snap(&[100, 300], 0, 20), (20, false));
    }

    #[test]
    fn classes_follow_the_design_steps() {
        assert_eq!(class(149), "Small");
        assert_eq!(class(150), "Medium");
        assert_eq!(class(300), "Large");
    }

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
