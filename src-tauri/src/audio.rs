//! Playback service for the presentation adapter. Called only on the main thread.
extern "C" {
    fn sw_play_cling() -> i32;
    fn sw_stop_cling();
}
pub fn play() -> bool {
    unsafe { sw_play_cling() != 0 }
}
pub fn stop() {
    unsafe { sw_stop_cling() }
}
