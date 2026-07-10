use hobolib::prln;
use crate::core::Core;

mod core;
mod glob;

fn main() {
    prln!("Hello, world!");
    // Запуск всех нитей приложения.
    Core::new();

}   // main()

// /// дитто
// pub fn request_shutdown() {
//     glob::STATE.lock().unwrap().is_app_stop_pending = true;
// }
//
// /// дитто
// pub fn is_shutdown_requested() -> bool {
//     glob::STATE.lock().unwrap().is_app_stop_pending
// }
