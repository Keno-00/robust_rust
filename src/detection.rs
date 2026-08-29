use crate::config::{HsvRange, PixelAcquisition, ProfileConfig, RectCompare};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Target {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub aspect: f32,
    pub seen: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Blob {
    pub min_x: usize,
    pub max_x: usize,
    pub min_y: usize,
    pub max_y: usize,
    pub area: usize,
    pub sum_x: usize,
    pub sum_y: usize,
    pub highest_x: usize,
    pub highest_y: usize,
}

#[derive(Debug, Clone)]
pub struct DetectionWorkspace {
    pub hsv: Vec<u8>,
    pub mask: Vec<u8>,
    pub buf1: Vec<u8>,
    pub buf2: Vec<u8>,
    pub labels: Vec<u32>,
    pub bgr_data: Vec<u8>,
    pub parent: Vec<u32>,
    pub temp_buf: Vec<u8>,
    pub label_map: Vec<i32>,
    pub blobs: Vec<Blob>,
}

impl DetectionWorkspace {
    pub fn new() -> Self {
        Self {
            hsv: Vec::new(),
            mask: Vec::new(),
            buf1: Vec::new(),
            buf2: Vec::new(),
            labels: Vec::new(),
            bgr_data: Vec::new(),
            parent: Vec::new(),
            temp_buf: Vec::new(),
            label_map: Vec::new(),
            blobs: Vec::new(),
        }
    }
}

pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let max_val = r.max(g).max(b);
    let min_val = r.min(g).min(b);
    let delta = max_val - min_val;
    
    let mut h = 0.0f32;
    if delta > 0 {
        let delta_f = delta as f32;
        if max_val == r {
            h = 60.0 * (((g as f32 - b as f32) / delta_f) % 6.0);
        } else if max_val == g {
            h = 60.0 * (((b as f32 - r as f32) / delta_f) + 2.0);
        } else {
            h = 60.0 * (((r as f32 - g as f32) / delta_f) + 4.0);
        }
        if h < 0.0 {
            h += 360.0;
        }
    }
    
    let s_val = if max_val == 0 {
        0
    } else {
        ((delta as u32 * 255 + max_val as u32 / 2) / max_val as u32) as u8
    };
    
    (
        (h / 2.0).round() as u8,
        s_val,
        max_val,
    )
}

fn dilate(src: &[u8], dst: &mut [u8], temp: &mut Vec<u8>, width: usize, height: usize) {
    if width == 0 || height == 0 {
        return;
    }
    temp.resize(width * height, 0);
    
    // Horizontal pass: dilate horizontally by 1 pixel in each direction
    for y in 0..height {
        let row_offset = y * width;
        if width == 1 {
            temp[row_offset] = src[row_offset];
            continue;
        }
        
        temp[row_offset] = src[row_offset].max(src[row_offset + 1]);
        
        for x in 1..(width - 1) {
            let idx = row_offset + x;
            temp[idx] = src[idx].max(src[idx - 1]).max(src[idx + 1]);
        }
        
        temp[row_offset + width - 1] = src[row_offset + width - 1].max(src[row_offset + width - 2]);
    }

    // Vertical pass: dilate vertically by 1 pixel in each direction
    for x in 0..width {
        if height == 1 {
            for y in 0..height {
                dst[y * width + x] = temp[y * width + x];
            }
            continue;
        }
        
        dst[x] = temp[x].max(temp[width + x]);
        
        for y in 1..(height - 1) {
            let idx = y * width + x;
            dst[idx] = temp[idx].max(temp[idx - width]).max(temp[idx + width]);
        }
        
        let last_row = (height - 1) * width;
        dst[last_row + x] = temp[last_row + x].max(temp[last_row - width + x]);
    }
}

fn erode(src: &[u8], dst: &mut [u8], temp: &mut Vec<u8>, width: usize, height: usize) {
    if width == 0 || height == 0 {
        return;
    }
    temp.resize(width * height, 0);
    
    // Horizontal pass: erode horizontally by 1 pixel in each direction
    for y in 0..height {
        let row_offset = y * width;
        if width == 1 {
            temp[row_offset] = src[row_offset];
            continue;
        }
        
        temp[row_offset] = src[row_offset].min(src[row_offset + 1]);
        
        for x in 1..(width - 1) {
            let idx = row_offset + x;
            temp[idx] = src[idx].min(src[idx - 1]).min(src[idx + 1]);
        }
        
        temp[row_offset + width - 1] = src[row_offset + width - 1].min(src[row_offset + width - 2]);
    }

    // Vertical pass: erode vertically by 1 pixel in each direction
    for x in 0..width {
        if height == 1 {
            for y in 0..height {
                dst[y * width + x] = temp[y * width + x];
            }
            continue;
        }
        
        dst[x] = temp[x].min(temp[width + x]);
        
        for y in 1..(height - 1) {
            let idx = y * width + x;
            dst[idx] = temp[idx].min(temp[idx - width]).min(temp[idx + width]);
        }
        
        let last_row = (height - 1) * width;
        dst[last_row + x] = temp[last_row + x].min(temp[last_row - width + x]);
    }
}

// Two-pass Connected Component Labeling
pub fn find_blobs(
    mask: &[u8],
    width: usize,
    height: usize,
    labels: &mut Vec<u32>,
    parent: &mut Vec<u32>,
    label_map: &mut Vec<i32>,
    blobs: &mut Vec<Blob>,
) {
    labels.resize(mask.len(), 0);
    labels.fill(0);
    let mut next_label = 1u32;
    parent.clear();
    parent.push(0u32); // label 0 is background

    fn find(parent: &mut Vec<u32>, mut i: u32) -> u32 {
        while parent[i as usize] != i {
            parent[i as usize] = parent[parent[i as usize] as usize];
            i = parent[i as usize];
        }
        i
    }

    fn union(parent: &mut Vec<u32>, i: u32, j: u32) {
        let root_i = find(parent, i);
        let root_j = find(parent, j);
        if root_i != root_j {
            parent[root_i as usize] = root_j;
        }
    }

    // First pass
    for y in 0..height {
        let offset = y * width;
        for x in 0..width {
            if mask[offset + x] != 0 {
                let mut neighbor_labels = [0u32; 4];
                let mut count = 0;
                if x > 0 && labels[offset + x - 1] != 0 {
                    neighbor_labels[count] = labels[offset + x - 1];
                    count += 1;
                }
                if y > 0 {
                    let up_offset = (y - 1) * width;
                    if labels[up_offset + x] != 0 {
                        neighbor_labels[count] = labels[up_offset + x];
                        count += 1;
                    }
                    if x > 0 && labels[up_offset + x - 1] != 0 {
                        neighbor_labels[count] = labels[up_offset + x - 1];
                        count += 1;
                    }
                    if x + 1 < width && labels[up_offset + x + 1] != 0 {
                        neighbor_labels[count] = labels[up_offset + x + 1];
                        count += 1;
                    }
                }

                if count == 0 {
                    labels[offset + x] = next_label;
                    parent.push(next_label);
                    next_label += 1;
                } else {
                    let mut min_l = neighbor_labels[0];
                    for i in 1..count {
                        if neighbor_labels[i] < min_l {
                            min_l = neighbor_labels[i];
                        }
                    }
                    labels[offset + x] = min_l;
                    for i in 0..count {
                        union(parent, neighbor_labels[i], min_l);
                    }
                }
            }
        }
    }

    // Second pass
    label_map.resize(next_label as usize, -1);
    label_map.fill(-1);
    let mut consecutive_id = 0usize;
    blobs.clear();

    for y in 0..height {
        let offset = y * width;
        for x in 0..width {
            let mut l = labels[offset + x];
            if l != 0 {
                l = find(parent, l);
                labels[offset + x] = l;

                let mapped_id = label_map[l as usize];
                let id = if mapped_id >= 0 {
                    mapped_id as usize
                } else {
                    let new_id = consecutive_id;
                    consecutive_id += 1;
                    label_map[l as usize] = new_id as i32;
                    blobs.push(Blob {
                        min_x: x,
                        max_x: x,
                        min_y: y,
                        max_y: y,
                        area: 0,
                        sum_x: 0,
                        sum_y: 0,
                        highest_x: x,
                        highest_y: y,
                    });
                    new_id
                };

                let blob = &mut blobs[id];
                blob.min_x = blob.min_x.min(x);
                blob.max_x = blob.max_x.max(x);
                blob.min_y = blob.min_y.min(y);
                blob.max_y = blob.max_y.max(y);
                blob.area += 1;
                blob.sum_x += x;
                blob.sum_y += y;
            }
        }
    }
}

fn calculate_distance(p1: (f64, f64), p2: (f64, f64)) -> f64 {
    ((p2.0 - p1.0).powi(2) + (p2.1 - p1.1).powi(2)).sqrt()
}

pub fn detect_target(
    img: &[u8],
    width: usize,
    height: usize,
    config: &ProfileConfig,
    ws: &mut DetectionWorkspace,
) -> Option<Target> {
    let (mut w, mut h) = (width, height);
    let bgr = if config.downscale && config.downscale_factor > 0.0 && config.downscale_factor < 1.0 {
        let factor = config.downscale_factor as f64;
        let new_w = (width as f64 * factor) as usize;
        let new_h = (height as f64 * factor) as usize;
        ws.bgr_data.resize(new_w * new_h * 3, 0);
        
        for y in 0..new_h {
            let src_y = (((y as f64) / factor) as usize).min(height - 1);
            let src_offset = src_y * width * 3;
            let dst_offset = y * new_w * 3;
            for x in 0..new_w {
                let src_x = (((x as f64) / factor) as usize).min(width - 1);
                let src_pixel = src_offset + src_x * 3;
                let dst_pixel = dst_offset + x * 3;
                ws.bgr_data[dst_pixel] = img[src_pixel];
                ws.bgr_data[dst_pixel + 1] = img[src_pixel + 1];
                ws.bgr_data[dst_pixel + 2] = img[src_pixel + 2];
            }
        }
        w = new_w;
        h = new_h;
        &ws.bgr_data
    } else {
        img
    };

    ws.hsv.resize(w * h * 3, 0);
    for i in 0..(w * h) {
        let b = bgr[i * 3];
        let g = bgr[i * 3 + 1];
        let r = bgr[i * 3 + 2];
        let (h_val, s_val, v_val) = rgb_to_hsv(b, g, r);
        ws.hsv[i * 3] = h_val;
        ws.hsv[i * 3 + 1] = s_val;
        ws.hsv[i * 3 + 2] = v_val;
    }

    let in_range = |h_val: u8, s_val: u8, v_val: u8, range: HsvRange| -> bool {
        h_val >= range.lower[0]
            && h_val <= range.upper[0]
            && s_val >= range.lower[1]
            && s_val <= range.upper[1]
            && v_val >= range.lower[2]
            && v_val <= range.upper[2]
    };

    // Constant range 4 from python
    let l4 = HsvRange::new([87, 121, 136], [147, 181, 196]);

    ws.mask.resize(w * h, 0);
    for i in 0..(w * h) {
        let h_val = ws.hsv[i * 3];
        let s_val = ws.hsv[i * 3 + 1];
        let v_val = ws.hsv[i * 3 + 2];

        let mut matches = in_range(h_val, s_val, v_val, config.aimbot_hsv[0])
            || in_range(h_val, s_val, v_val, config.aimbot_hsv[1])
            || in_range(h_val, s_val, v_val, config.aimbot_hsv[2])
            || in_range(h_val, s_val, v_val, l4);

        if config.pixel_acquisition == PixelAcquisition::Edging {
            // Edging checks 3 ranges only (no l4)
            matches = in_range(h_val, s_val, v_val, config.aimbot_hsv[0])
                || in_range(h_val, s_val, v_val, config.aimbot_hsv[1])
                || in_range(h_val, s_val, v_val, config.aimbot_hsv[2]);
        }

        ws.mask[i] = if matches { 255 } else { 0 };
    }

    let midwidth = w as f64 / 2.0;
    let midheight = h as f64 / 2.0;
    let image_center = (midwidth, midheight);

    match config.pixel_acquisition {
        PixelAcquisition::Contouring => {
            // Morphology: 9 dilations + 1 erosion (optimized separable)
            ws.buf1.resize(w * h, 0);
            ws.buf1.copy_from_slice(&ws.mask);
            ws.buf2.resize(w * h, 0);
            for _ in 0..9 {
                dilate(&ws.buf1, &mut ws.buf2, &mut ws.temp_buf, w, h);
                std::mem::swap(&mut ws.buf1, &mut ws.buf2);
            }
            erode(&ws.buf1, &mut ws.buf2, &mut ws.temp_buf, w, h);

            find_blobs(&ws.buf2, w, h, &mut ws.labels, &mut ws.parent, &mut ws.label_map, &mut ws.blobs);
            let blobs = &ws.blobs;
            if blobs.is_empty() {
                return None;
            }

            let mut targets = Vec::new();
            for blob in blobs {
                let bw = (blob.max_x - blob.min_x + 1) as i32;
                let bh = (blob.max_y - blob.min_y + 1) as i32;
                let aspect = (bw as f32 / bh as f32) - 1.0;

                if config.rect_filter {
                    match config.rect_compare {
                        RectCompare::LessThan => {
                            if aspect < config.rect_ratio {
                                return None; // Return None immediately matching Python
                            }
                        }
                        RectCompare::GreaterThan => {
                            if aspect > config.rect_ratio {
                                return None;
                            }
                        }
                    }
                }

                if config.area_filter {
                    let area = blob.area as i32;
                    if area < config.area_min || area > config.area_max {
                        return None; // Return None immediately matching Python
                    }
                }

                let (c_x, c_y) = if !config.highest_pixel {
                    (
                        blob.sum_x as f64 / blob.area as f64,
                        blob.sum_y as f64 / blob.area as f64,
                    )
                } else {
                    (blob.highest_x as f64, blob.highest_y as f64)
                };

                let dist = calculate_distance(image_center, (c_x, c_y));
                targets.push((c_x, c_y, bw, bh, aspect, dist));
            }

            targets.sort_by(|a, b| a.5.partial_cmp(&b.5).unwrap());
            let best = targets[0];
            
            let mut target_x = best.0 - midwidth;
            let mut target_y = best.1 - midheight;
            if config.downscale && config.downscale_factor > 0.0 {
                let factor = config.downscale_factor as f64;
                target_x /= factor;
                target_y /= factor;
            }

            Some(Target {
                x: target_x.round() as i32,
                y: target_y.round() as i32,
                width: best.2,
                height: best.3,
                aspect: best.4,
                seen: true,
            })
        }
        PixelAcquisition::Edging => {
            // Edging threshold
            find_blobs(&ws.mask, w, h, &mut ws.labels, &mut ws.parent, &mut ws.label_map, &mut ws.blobs);
            let blobs = &ws.blobs;
            if blobs.is_empty() {
                return None;
            }

            let mut x_min = usize::MAX;
            let mut x_max = 0;

            for blob in blobs {
                x_min = x_min.min(blob.min_x);
                x_max = x_max.max(blob.max_x);
            }

            if x_min > x_max {
                return None;
            }

            let n_x = (x_min + x_max) as f64 / 2.0;
            let mut target_x = n_x - midwidth;
            if config.downscale && config.downscale_factor > 0.0 {
                let factor = config.downscale_factor as f64;
                target_x /= factor;
            }

            Some(Target {
                x: target_x.round() as i32,
                y: 0,
                width: (x_max - x_min + 1) as i32,
                height: 1,
                aspect: 0.0,
                seen: true,
            })
        }
    }
}

pub fn detect_target_merged(
    img: &[u8],
    width: usize,
    height: usize,
    config: &ProfileConfig,
    ws: &mut DetectionWorkspace,
) -> Option<Target> {
    // Port of countcolm
    let (mut w, mut h) = (width, height);
    let bgr = if config.downscale && config.downscale_factor > 0.0 && config.downscale_factor < 1.0 {
        let factor = config.downscale_factor as f64;
        let new_w = (width as f64 * factor) as usize;
        let new_h = (height as f64 * factor) as usize;
        ws.bgr_data.resize(new_w * new_h * 3, 0);
        
        for y in 0..new_h {
            let src_y = (((y as f64) / factor) as usize).min(height - 1);
            let src_offset = src_y * width * 3;
            let dst_offset = y * new_w * 3;
            for x in 0..new_w {
                let src_x = (((x as f64) / factor) as usize).min(width - 1);
                let src_pixel = src_offset + src_x * 3;
                let dst_pixel = dst_offset + x * 3;
                ws.bgr_data[dst_pixel] = img[src_pixel];
                ws.bgr_data[dst_pixel + 1] = img[src_pixel + 1];
                ws.bgr_data[dst_pixel + 2] = img[src_pixel + 2];
            }
        }
        w = new_w;
        h = new_h;
        &ws.bgr_data
    } else {
        img
    };

    ws.hsv.resize(w * h * 3, 0);
    for i in 0..(w * h) {
        let b = bgr[i * 3];
        let g = bgr[i * 3 + 1];
        let r = bgr[i * 3 + 2];
        let (h_val, s_val, v_val) = rgb_to_hsv(b, g, r);
        ws.hsv[i * 3] = h_val;
        ws.hsv[i * 3 + 1] = s_val;
        ws.hsv[i * 3 + 2] = v_val;
    }

    let in_range = |h_val: u8, s_val: u8, v_val: u8, range: HsvRange| -> bool {
        h_val >= range.lower[0]
            && h_val <= range.upper[0]
            && s_val >= range.lower[1]
            && s_val <= range.upper[1]
            && v_val >= range.lower[2]
            && v_val <= range.upper[2]
    };

    ws.mask.resize(w * h, 0);
    for i in 0..(w * h) {
        let h_val = ws.hsv[i * 3];
        let s_val = ws.hsv[i * 3 + 1];
        let v_val = ws.hsv[i * 3 + 2];

        let matches = in_range(h_val, s_val, v_val, config.aimbot_hsv[0])
            || in_range(h_val, s_val, v_val, config.aimbot_hsv[1])
            || in_range(h_val, s_val, v_val, config.aimbot_hsv[2]);

        ws.mask[i] = if matches { 255 } else { 0 };
    }

    // Morphology
    ws.buf1.resize(w * h, 0);
    ws.buf1.copy_from_slice(&ws.mask);
    ws.buf2.resize(w * h, 0);
    for _ in 0..9 {
        dilate(&ws.buf1, &mut ws.buf2, &mut ws.temp_buf, w, h);
        std::mem::swap(&mut ws.buf1, &mut ws.buf2);
    }
    erode(&ws.buf1, &mut ws.buf2, &mut ws.temp_buf, w, h);

    find_blobs(&ws.buf2, w, h, &mut ws.labels, &mut ws.parent, &mut ws.label_map, &mut ws.blobs);
    let blobs = &ws.blobs;
    if blobs.is_empty() {
        return None;
    }

    let bounding_boxes: Vec<(i32, i32, i32, i32)> = blobs.iter().map(|b| {
        (
            b.min_x as i32,
            b.min_y as i32,
            (b.max_x - b.min_x + 1) as i32,
            (b.max_y - b.min_y + 1) as i32,
        )
    }).collect();

    let are_boxes_close = |box1: (i32, i32, i32, i32), box2: (i32, i32, i32, i32), threshold: i32| -> bool {
        let horizontal_distance = 0.max(box2.0 - (box1.0 + box1.2)).max(box1.0 - (box2.0 + box2.2));
        let vertical_distance = 0.max(box2.1 - (box1.1 + box1.3)).max(box1.1 - (box2.1 + box2.3));
        horizontal_distance <= threshold && vertical_distance <= threshold
    };

    // Merge bounding boxes
    let mut merged_boxes = Vec::new();
    let mut merged = vec![false; bounding_boxes.len()];
    for i in 0..bounding_boxes.len() {
        if merged[i] {
            continue;
        }
        let mut cluster = bounding_boxes[i];
        for j in 0..bounding_boxes.len() {
            if i != j && !merged[j] && are_boxes_close(cluster, bounding_boxes[j], config.merge_distance) {
                cluster = (
                    cluster.0.min(bounding_boxes[j].0),
                    cluster.1.min(bounding_boxes[j].1),
                    (cluster.0 + cluster.2).max(bounding_boxes[j].0 + bounding_boxes[j].2) - cluster.0.min(bounding_boxes[j].0),
                    (cluster.1 + cluster.3).max(bounding_boxes[j].1 + bounding_boxes[j].3) - cluster.1.min(bounding_boxes[j].1),
                );
                merged[j] = true;
            }
        }
        merged_boxes.push(cluster);
    }

    let midwidth = w as f64 / 2.0;
    let midheight = h as f64 / 2.0;
    let image_center = (midwidth, midheight);
    let mut targets = Vec::new();

    for box_item in merged_boxes {
        let c_x = box_item.0 as f64 + box_item.2 as f64 / 2.0;
        let c_y = box_item.1 as f64 + box_item.3 as f64 / 2.0;
        let dist = calculate_distance(image_center, (c_x, c_y));
        targets.push((c_x, c_y, box_item.2, box_item.3, dist));
    }

    targets.sort_by(|a, b| a.4.partial_cmp(&b.4).unwrap());
    let best = targets[0];

    let mut target_x = best.0 - midwidth;
    let mut target_y = best.1 - midheight;
    if config.downscale && config.downscale_factor > 0.0 {
        let factor = config.downscale_factor as f64;
        target_x /= factor;
        target_y /= factor;
    }

    Some(Target {
        x: target_x.round() as i32,
        y: target_y.round() as i32,
        width: best.2,
        height: best.3,
        aspect: 0.0,
        seen: true,
    })
}

pub fn partition_aim_image(
    img: &[u8],
    width: usize,
    height: usize,
) -> (Vec<u8>, usize, usize) {
    let cut_width = 5;
    if width < cut_width || height < cut_width {
        return (img.to_vec(), width, height);
    }

    let center_x = width / 2;
    let center_y = height / 2;

    let new_w = width - cut_width;
    let new_h = center_y - cut_width / 2;

    let mut new_buf = vec![0u8; new_w * new_h * 3];

    for y in 0..new_h {
        let orig_y = y;
        let orig_offset = orig_y * width * 3;
        let new_offset = y * new_w * 3;
        for x in 0..new_w {
            let orig_x = if x < center_x - cut_width / 2 {
                x
            } else {
                x + cut_width
            };

            let src_idx = orig_offset + orig_x * 3;
            let dst_idx = new_offset + x * 3;

            new_buf[dst_idx] = img[src_idx];
            new_buf[dst_idx + 1] = img[src_idx + 1];
            new_buf[dst_idx + 2] = img[src_idx + 2];
        }
    }

    (new_buf, new_w, new_h)
}

pub fn check_trigger_color(
    img: &[u8],
    width: usize,
    height: usize,
    config: &ProfileConfig,
) -> bool {
    let cut_width = 5;
    if width < cut_width || height < cut_width {
        return false;
    }

    let center_y = height / 2;

    let w_v = width - cut_width;
    let partright_start_v = w_v / 2 + cut_width / 2;
    let w_pr = w_v - partright_start_v;
    let w_pl1 = w_pr / 2 - cut_width / 2;
    let w_pl2 = w_pl1 / 2 - cut_width / 2;

    if w_pl2 == 0 {
        return false;
    }

    let x_start = partright_start_v + cut_width;
    let y_start = center_y - cut_width / 2;

    let l4 = [87, 121, 136];
    let u4 = [147, 181, 196];
    let l5 = [90, 196, 210];
    let u5 = [148, 255, 255];

    let in_range = |h_val: u8, s_val: u8, v_val: u8, lower: [u8; 3], upper: [u8; 3]| -> bool {
        h_val >= lower[0]
            && h_val <= upper[0]
            && s_val >= lower[1]
            && s_val <= upper[1]
            && v_val >= lower[2]
            && v_val <= upper[2]
    };

    for y_offset in 0..cut_width {
        let y = y_start + y_offset;
        if y >= height {
            continue;
        }
        let orig_offset = y * width * 3;
        for x_offset in 0..w_pl2 {
            let x = x_start + x_offset;
            if x >= width {
                continue;
            }

            let pixel_idx = orig_offset + x * 3;
            let b = img[pixel_idx];
            let g = img[pixel_idx + 1];
            let r = img[pixel_idx + 2];

            let (h, s, v) = rgb_to_hsv(b, g, r);

            let matches = in_range(h, s, v, config.aimbot_hsv[0].lower, config.aimbot_hsv[0].upper)
                || in_range(h, s, v, config.aimbot_hsv[1].lower, config.aimbot_hsv[1].upper)
                || in_range(h, s, v, config.aimbot_hsv[2].lower, config.aimbot_hsv[2].upper)
                || in_range(h, s, v, l4, u4)
                || in_range(h, s, v, l5, u5);

            if matches {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProfileConfig;

    #[test]
    fn test_rgb_to_hsv() {
        let (h, s, v) = rgb_to_hsv(255, 0, 0); // passing pure Red BGR values
        assert_eq!(h, 0);
        assert_eq!(s, 255);
        assert_eq!(v, 255);
    }

    #[test]
    fn test_find_blobs_single() {
        let mut mask = vec![0u8; 100];
        mask[11] = 255;
        mask[12] = 255;
        mask[21] = 255;
        
        let mut ws = DetectionWorkspace::new();
        find_blobs(&mask, 10, 10, &mut ws.labels, &mut ws.parent, &mut ws.label_map, &mut ws.blobs);
        let blobs = &ws.blobs;
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].area, 3);
        assert_eq!(blobs[0].min_x, 1);
        assert_eq!(blobs[0].max_x, 2);
    }

    #[test]
    fn test_detect_target_simple() {
        // Create a 10x10 dummy RGB image with a target-colored pixel at (6, 6)
        let mut img = vec![0u8; 100 * 3];
        // B=20, G=20, R=240 produces H=120, S=234, V=240 which fits perfectly.
        let pixel_idx = 6 * 10 + 6;
        img[pixel_idx * 3] = 20;
        img[pixel_idx * 3 + 1] = 20;
        img[pixel_idx * 3 + 2] = 240;

        let mut config = ProfileConfig::default();
        config.pixel_acquisition = PixelAcquisition::Contouring;
        config.rect_filter = false;
        config.area_filter = false;

        let mut ws = DetectionWorkspace::new();
        let target = detect_target(&img, 10, 10, &config, &mut ws);
        assert!(target.is_some());
        let t = target.unwrap();
        assert!(t.seen);
    }

    #[test]
    fn test_partition_aim_image() {
        let img = vec![0u8; 100 * 3];
        let (part_buf, part_w, part_h) = partition_aim_image(&img, 10, 10);
        assert_eq!(part_w, 5);
        assert_eq!(part_h, 3);
        assert_eq!(part_buf.len(), 5 * 3 * 3);
    }

    #[test]
    fn test_check_trigger_color() {
        let mut config = ProfileConfig::default();
        config.aimbot_hsv[0] = HsvRange::new([105, 214, 240], [135, 244, 255]);

        let mut img_large = vec![0u8; 100 * 100 * 3];
        // For 100x100: x_start = 54, y_start = 48
        // Place pixel at x=56, y=50. Color BGR: 20, 20, 240 (produces HSV: 120, 234, 240)
        let idx = (50 * 100 + 56) * 3;
        img_large[idx] = 20;     // B
        img_large[idx + 1] = 20; // G
        img_large[idx + 2] = 240;// R

        assert!(check_trigger_color(&img_large, 100, 100, &config));
    }
}
