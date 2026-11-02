//! Checked row-vector projection for the observed APB camera layout.
//! Actor discovery and target eligibility are separate from this math.

#[derive(Debug, Clone, Copy)]
pub struct CameraProjection {
    /// APB world position read from camera object +0x6C0.
    pub position: [f32; 3],
    /// Row-major 4x4 block read from camera object +0x680.
    pub matrix: [f32; 16],
}

#[derive(Debug, Clone, Copy)]
pub struct ClientPoint {
    pub x: f64,
    pub y: f64,
    pub depth: f64,
}

impl CameraProjection {
    /// Returns a client-relative pixel coordinate, or `None` when the point is
    /// behind the camera or outside the normalized view. Invalid data is an
    /// error rather than a request to try another coordinate source.
    pub fn project(
        &self,
        world: [f32; 3],
        client_width: u32,
        client_height: u32,
    ) -> Result<Option<ClientPoint>, String> {
        Ok(self
            .project_unclipped(world, client_width, client_height)?
            .filter(|point| {
                point.x >= 0.0
                    && point.x <= f64::from(client_width)
                    && point.y >= 0.0
                    && point.y <= f64::from(client_height)
            }))
    }

    /// Bounds may extend off screen while part of the character remains visible.
    pub fn project_unclipped(
        &self,
        world: [f32; 3],
        client_width: u32,
        client_height: u32,
    ) -> Result<Option<ClientPoint>, String> {
        if client_width == 0 || client_height == 0 {
            return Err("APB client viewport has zero width or height.".to_string());
        }
        if !self
            .position
            .iter()
            .chain(self.matrix.iter())
            .chain(world.iter())
            .all(|value| value.is_finite())
        {
            return Err("APB camera or world position contains a nonfinite value.".to_string());
        }

        let relative = [
            f64::from(world[0]) - f64::from(self.position[0]),
            f64::from(world[1]) - f64::from(self.position[1]),
            f64::from(world[2]) - f64::from(self.position[2]),
            1.0,
        ];
        let mut clip = [0.0f64; 4];
        for (column, value) in clip.iter_mut().enumerate() {
            *value = (0..4)
                .map(|row| relative[row] * f64::from(self.matrix[row * 4 + column]))
                .sum();
        }
        if !clip.iter().all(|value| value.is_finite()) {
            return Err("APB projection produced a nonfinite clip coordinate.".to_string());
        }
        if clip[3] <= 0.0 {
            return Ok(None);
        }
        let ndc_x = clip[0] / clip[3];
        let ndc_y = clip[1] / clip[3];
        Ok(Some(ClientPoint {
            x: (ndc_x + 1.0) * f64::from(client_width) / 2.0,
            y: (1.0 - ndc_y) * f64::from(client_height) / 2.0,
            depth: clip[3],
        }))
    }
}
