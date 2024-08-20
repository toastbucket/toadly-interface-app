mod app;
mod usb;
mod vedirect;

use app::App;

fn main() {
    let mut app = App::new();
    app.init();
    app.run();
}
