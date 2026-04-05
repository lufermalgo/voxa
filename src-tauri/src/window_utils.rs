use tauri::{PhysicalPosition, PhysicalSize};

/// Calculates the position for the dictation pill to be centered horizontally
/// at the bottom of the screen with a specific padding.
pub fn calculate_pill_position(
    monitor_size: PhysicalSize<u32>,
    monitor_pos: PhysicalPosition<i32>,
    window_size: PhysicalSize<u32>,
    padding_bottom: i32,
) -> PhysicalPosition<i32> {
    let x = monitor_pos.x + (monitor_size.width as i32 / 2) - (window_size.width as i32 / 2);
    let y = monitor_pos.y + monitor_size.height as i32 - window_size.height as i32 - padding_bottom;
    PhysicalPosition::new(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_pill_position() {
        let monitor_size = PhysicalSize::new(1920, 1080);
        let monitor_pos = PhysicalPosition::new(0, 0);
        let window_size = PhysicalSize::new(300, 100);
        let padding = 15;

        // x = 0 + (1920/2) - (300/2) = 960 - 150 = 810
        // y = 0 + 1080 - 100 - 15 = 965
        let pos = calculate_pill_position(monitor_size, monitor_pos, window_size, padding);
        assert_eq!(pos.x, 810);
        assert_eq!(pos.y, 965);
    }

    #[test]
    fn test_calculate_pill_position_offset_monitor() {
        let monitor_size = PhysicalSize::new(1920, 1080);
        let monitor_pos = PhysicalPosition::new(1920, 0); // Second monitor
        let window_size = PhysicalSize::new(300, 100);
        let padding = 20;

        // x = 1920 + 960 - 150 = 2730
        // y = 0 + 1080 - 100 - 20 = 960
        let pos = calculate_pill_position(monitor_size, monitor_pos, window_size, padding);
        assert_eq!(pos.x, 2730);
        assert_eq!(pos.y, 960);
    }
}
