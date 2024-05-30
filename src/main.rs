slint::include_modules!();

fn main() {
    let mw = MainWindow::new().unwrap();

    mw.set_house_batt_level(0.142);
    mw.set_truck_batt_voltage(12.5642);
    mw.set_fresh_tank_level(0.9);
    mw.set_grey_tank_level(0.2);

    mw.run().unwrap();
}
