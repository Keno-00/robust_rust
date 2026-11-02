//! Fixed APB name detection from packed BGR canvas pixels.

use crate::detection::{find_blobs, Blob, Target};

pub const UNICORN_RED_MIN: u8 = 240;
pub const PEGASUS_RED_MIN: u8 = 204; // 15% below the unicorn brightness threshold.
pub const RED_DOMINANCE_MIN: u8 = 96;
pub const HORIZONTAL_REDUCTION: usize = 2;
pub const CROSSHAIR_CUT_WIDTH: usize = 5;
pub const MAX_LETTER_GAP: usize = 6; // Original canvas pixels, independent of reduction.
pub const MAX_DETACHED_STROKE_GAP: usize = 2;
pub const MIN_NAME_RED_PIXELS: usize = 4;
pub const MIN_NAME_EXTENT: usize = 3;

#[derive(Clone, Copy)]
struct Bounds {
    min_x: usize,
    max_x: usize,
    min_y: usize,
    max_y: usize,
    red_pixels: usize,
}

impl Bounds {
    fn height(self) -> usize {
        self.max_y - self.min_y + 1
    }

    fn include(&mut self, other: Self) {
        self.min_x = self.min_x.min(other.min_x);
        self.max_x = self.max_x.max(other.max_x);
        self.min_y = self.min_y.min(other.min_y);
        self.max_y = self.max_y.max(other.max_y);
        self.red_pixels += other.red_pixels;
    }
}

#[derive(Default)]
pub struct ApbDetectionWorkspace {
    mask: Vec<u8>,
    labels: Vec<u32>,
    parent: Vec<u32>,
    label_map: Vec<i32>,
    blobs: Vec<Blob>,
    components: Vec<Bounds>,
    group_roots: Vec<usize>,
    names: Vec<Option<Bounds>>,
}

fn is_red(bgr: &[u8], red_min: u8) -> bool {
    let red = bgr[2];
    red >= red_min
        && red.saturating_sub(bgr[1]) >= RED_DOMINANCE_MIN
        && red.saturating_sub(bgr[0]) >= RED_DOMINANCE_MIN
}

fn valid_image(img: &[u8], width: usize, height: usize) -> bool {
    width > 0
        && height > 0
        && width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(3))
            .is_some_and(|length| img.len() >= length)
}

fn same_name(left: Bounds, right: Bounds) -> bool {
    let overlap_x = left.min_x.max(right.min_x) <= left.max_x.min(right.max_x);
    let overlap_top = left.min_y.max(right.min_y);
    let overlap_bottom = left.max_y.min(right.max_y);
    let overlap_y = (overlap_bottom + 1).saturating_sub(overlap_top);
    let horizontal_gap = right.min_x.saturating_sub(left.max_x + 1);
    let letter_gap = (left.height().max(right.height()) / 2).clamp(2, MAX_LETTER_GAP);

    // Join aligned letters using their original bounds, rather than expanding a cluster
    // until it reaches another name. Y remains useful for separating lines of text.
    if horizontal_gap <= letter_gap
        && overlap_y * 2 >= left.height().min(right.height())
        && left.max_y.abs_diff(right.max_y) <= left.height().max(right.height()) / 2
    {
        return true;
    }

    // Attach tiny detached strokes (e.g. the dot of an i) only within the same X span.
    let vertical_gap = left
        .min_y
        .max(right.min_y)
        .saturating_sub(left.max_y.min(right.max_y) + 1);
    overlap_x
        && left.height().min(right.height()) <= MAX_DETACHED_STROKE_GAP
        && vertical_gap <= MAX_DETACHED_STROKE_GAP
}

fn root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        parents[index] = parents[parents[index]];
        index = parents[index];
    }
    index
}

pub fn detect_name(
    img: &[u8],
    width: usize,
    height: usize,
    ws: &mut ApbDetectionWorkspace,
) -> Option<Target> {
    if !valid_image(img, width, height) {
        return None;
    }

    // Preserve the existing upper-half search and crosshair exclusion, but do not
    // squeeze the two sides together into a copied image with shifted X coordinates.
    let partitioned = width >= CROSSHAIR_CUT_WIDTH && height >= CROSSHAIR_CUT_WIDTH;
    let search_height = if partitioned {
        (height / 2).saturating_sub(CROSSHAIR_CUT_WIDTH / 2)
    } else {
        height
    };
    let cut_start = (width / 2).saturating_sub(CROSSHAIR_CUT_WIDTH / 2);
    let cut_end = cut_start + CROSSHAIR_CUT_WIDTH;
    let mask_width = width.div_ceil(HORIZONTAL_REDUCTION);
    ws.mask.resize(mask_width * search_height, 0);
    ws.mask.fill(0);

    for y in 0..search_height {
        for x in 0..width {
            if partitioned && (cut_start..cut_end).contains(&x) {
                continue;
            }
            let pixel = (y * width + x) * 3;
            if is_red(&img[pixel..pixel + 3], UNICORN_RED_MIN) {
                // OR pooling also retains the source subpixel as a bit. Connected
                // components use nonzero values; those bits refine exact X bounds later.
                ws.mask[y * mask_width + x / HORIZONTAL_REDUCTION] |=
                    1 << (x % HORIZONTAL_REDUCTION);
            }
        }
    }

    find_blobs(
        &ws.mask,
        mask_width,
        search_height,
        &mut ws.labels,
        &mut ws.parent,
        &mut ws.label_map,
        &mut ws.blobs,
    );
    ws.components.clear();
    ws.components.extend(ws.blobs.iter().map(|blob| Bounds {
        min_x: usize::MAX,
        max_x: 0,
        min_y: blob.min_y,
        max_y: blob.max_y,
        red_pixels: 0,
    }));

    for (index, &bits) in ws.mask.iter().enumerate() {
        if bits == 0 {
            continue;
        }
        let component = ws.label_map[ws.labels[index] as usize] as usize;
        let bounds = &mut ws.components[component];
        let source_x = (index % mask_width) * HORIZONTAL_REDUCTION;
        let first = source_x + usize::from(bits & 1 == 0);
        let last = source_x + usize::from(bits & 2 != 0);
        bounds.min_x = bounds.min_x.min(first);
        bounds.max_x = bounds.max_x.max(last);
        bounds.red_pixels += bits.count_ones() as usize;
    }

    ws.components
        .sort_unstable_by_key(|bounds| (bounds.min_x, bounds.min_y));
    ws.group_roots.clear();
    ws.group_roots.extend(0..ws.components.len());
    for i in 0..ws.components.len() {
        for j in i + 1..ws.components.len() {
            if ws.components[j].min_x > ws.components[i].max_x + 1 + MAX_LETTER_GAP {
                break;
            }
            if same_name(ws.components[i], ws.components[j]) {
                let left = root(&mut ws.group_roots, i);
                let right = root(&mut ws.group_roots, j);
                ws.group_roots[right] = left;
            }
        }
    }

    ws.names.resize(ws.components.len(), None);
    ws.names.fill(None);
    for i in 0..ws.components.len() {
        let group = root(&mut ws.group_roots, i);
        match &mut ws.names[group] {
            Some(bounds) => bounds.include(ws.components[i]),
            slot @ None => *slot = Some(ws.components[i]),
        }
    }

    let mut best: Option<(u64, usize, Bounds)> = None;
    for bounds in ws.names.iter().flatten().copied() {
        if bounds.red_pixels < MIN_NAME_RED_PIXELS
            || bounds.max_x - bounds.min_x + 1 < MIN_NAME_EXTENT
            || bounds.height() < MIN_NAME_EXTENT
        {
            continue;
        }
        let distance = (bounds.min_x + bounds.max_x + 1).abs_diff(width);
        let tie_distance = (bounds.min_y + bounds.max_y + 1).abs_diff(height);
        let unicorn_distance = (distance as u64).pow(2) + (tie_distance as u64).pow(2);
        if best.is_none_or(|(best_distance, best_x, _)| {
            (unicorn_distance, distance) < (best_distance, best_x)
        }) {
            best = Some((unicorn_distance, distance, bounds));
        }
    }

    best.map(|(_, _, bounds)| Target {
        x: ((bounds.min_x as f64 + bounds.max_x as f64 + 1.0 - width as f64) / 2.0).round() as i32,
        y: ((bounds.min_y as f64 + bounds.max_y as f64 + 1.0 - height as f64) / 2.0).round() as i32,
        width: (bounds.max_x - bounds.min_x + 1) as i32,
        height: bounds.height() as i32,
        aspect: 0.0,
        seen: true,
    })
}

pub fn pegasus_has_red(img: &[u8], width: usize, height: usize) -> bool {
    if !valid_image(img, width, height)
        || width < CROSSHAIR_CUT_WIDTH
        || height < CROSSHAIR_CUT_WIDTH
    {
        return false;
    }
    // Keep the existing pegasus strip; only its color predicate changes.
    let remaining_width = width - CROSSHAIR_CUT_WIDTH;
    let right_start = remaining_width / 2 + CROSSHAIR_CUT_WIDTH / 2;
    let right_width = remaining_width.saturating_sub(right_start);
    let first_part_width = (right_width / 2).saturating_sub(CROSSHAIR_CUT_WIDTH / 2);
    let strip_width = (first_part_width / 2).saturating_sub(CROSSHAIR_CUT_WIDTH / 2);
    let start_x = (right_start + CROSSHAIR_CUT_WIDTH).min(width);
    let end_x = (start_x + strip_width).min(width);
    let start_y = height / 2 - CROSSHAIR_CUT_WIDTH / 2;
    let end_y = (start_y + CROSSHAIR_CUT_WIDTH).min(height);

    for y in start_y..end_y {
        for x in start_x..end_x {
            let pixel = (y * width + x) * 3;
            if is_red(&img[pixel..pixel + 3], PEGASUS_RED_MIN) {
                return true;
            }
        }
    }
    false
}
