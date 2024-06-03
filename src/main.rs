slint::include_modules!();

fn main() {
    let mw = MainWindow::new().unwrap();
    let mw_w = mw.as_weak();

    let _ = std::thread::spawn(move || {
        let mut house_batt = 1.0;
        let mut truck_batt = 0.0;
        let mut fresh_tank = 1.0;
        let mut grey_tank = 0.0;
        let mut inverter_value = 0.0;
        let mut solar_value = 0.0;

        loop {
            let _ = mw_w.upgrade_in_event_loop(move |mw| {
                mw.set_house_batt_level(house_batt);
                mw.set_truck_batt_voltage(truck_batt);
                mw.set_fresh_tank_level(fresh_tank);
                mw.set_grey_tank_level(grey_tank);
                mw.set_inverter_value(inverter_value);
                mw.set_solar_value(inverter_value);
            });

            if house_batt > 0.0 {
                house_batt -= 0.05;
            }

            if truck_batt < 14.0 {
                truck_batt += 0.5;
            }

            if fresh_tank > 0.0 {
                fresh_tank -= 0.05;
            }

            if grey_tank < 1.0 {
                grey_tank += 0.05;
            }

            if inverter_value < 1000.0 {
                inverter_value += 10.0;
            }

            if solar_value < 300.0 {
                solar_value += 10.0;
            }

            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
    });

    mw.run().unwrap();
}
