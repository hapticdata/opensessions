pub const MIN_SIDEBAR_WIDTH: u16 = 20;
pub const MAX_SIDEBAR_WIDTH: u16 = 80;

pub fn clamp_sidebar_width(width: u16) -> u16 {
    width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH)
}
