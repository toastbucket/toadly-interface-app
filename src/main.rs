slint::include_modules!();

fn main() {
    let mw = MainWindow::new().unwrap();

    mw.set_house_batt_level(0.4567);
    mw.set_fresh_tank_level(0.9);
    mw.set_grey_tank_level(0.2);

    mw.run().unwrap();
}
