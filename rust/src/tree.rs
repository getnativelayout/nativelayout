//! Port of NativeTree.kt. For now only `Length` and `Size`, which `css` needs.
//! The rest (Sides, Corners, NativeStyle, Native...) comes next.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Length {
    pub value: f32,
    pub scalable: bool,
}

impl Length {
    pub const ZERO: Length = Length { value: 0.0, scalable: false };

    pub fn rem(value: f32) -> Length {
        Length { value, scalable: true }
    }

    pub fn px(value: f32) -> Length {
        Length { value, scalable: false }
    }

    pub fn to_pixels(&self, density: f32, font_scale: f32) -> f32 {
        if self.scalable {
            self.value * 16.0 * density * font_scale
        } else {
            self.value * density
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    Dim(Length),
    Percent(f32),
    Auto,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_pixels() {
        assert_eq!(Length::px(10.0).to_pixels(2.0, 1.5), 20.0);
        assert_eq!(Length::rem(1.0).to_pixels(2.0, 1.0), 32.0);
        assert_eq!(Length::ZERO.to_pixels(3.0, 3.0), 0.0);
    }
}
