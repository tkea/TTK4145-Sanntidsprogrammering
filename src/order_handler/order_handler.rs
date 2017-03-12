#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

use elevator_driver::elev_io::*;

pub struct OrderHandler {
    orders_up: [bool; N_FLOORS],
    orders_down: [bool; N_FLOORS],
    orders_internal: [bool; N_FLOORS],
}

impl OrderHandler {

    pub fn new() -> Self {
        let orders = OrderHandler {
            orders_up: [false; N_FLOORS],
            orders_down: [false; N_FLOORS],
            orders_internal: [false; N_FLOORS],
        };
    return orders;
    }


    pub fn new_floor_order(&mut self, button: Button) {
        match button {
            Button::CallUp(Floor::At(floor)) => self.orders_up[floor] = true,
            Button::CallDown(Floor::At(floor)) => self.orders_down[floor] = true,
            Button::Internal(Floor::At(floor)) => self.orders_internal[floor] = true,
            _ => {}
        }
    }


    pub fn clear_orders_here(&mut self, floor: usize, direction: MotorDir) {
        self.orders_internal[floor] = false;

        match direction {
            MotorDir::Up => self.orders_up[floor] = false,
            MotorDir::Down => self.orders_down[floor] = false,
            _ => ()
        }
    }


    fn orders_in_direction(&self, floor: usize, direction: MotorDir) -> bool {
        let (lower_bound, num_elements) = match direction {
            MotorDir::Down => (0, floor),
            _ => (floor+1, N_FLOORS-floor+1),
        };

        let internal = self.orders_internal.iter().skip(lower_bound).take(num_elements);
        let up = self.orders_up.iter().skip(lower_bound).take(num_elements);
        let down = self.orders_down.iter().skip(lower_bound).take(num_elements);

        let mut orders = internal.chain(up).chain(down);

        if orders.any(|&floor_ordered| floor_ordered) {
            return true;
        } else {
            return false;
        }
    }


    pub fn should_stop(&self, floor: usize, direction: MotorDir) -> bool {
        let should_stop = match direction {
            MotorDir::Up => self.orders_up[floor] || self.orders_internal[floor],
            MotorDir::Down => self.orders_down[floor] || self.orders_internal[floor],
            _ => false
        };
        return should_stop;
    }


    pub fn should_continue(&self, floor: usize, direction: MotorDir) -> bool {
        return self.orders_in_direction(floor, direction);
    }


    pub fn should_change_direction(&self, floor: usize, direction: MotorDir) -> bool {
        let opposite_direction = match direction {
            MotorDir::Down => MotorDir::Up,
            _ => MotorDir::Down
        };

        if self.orders_in_direction(floor, opposite_direction) {
            return true;
        }

        // Handle edge case
        // where current_floor == 0 || current_floor == top floor
        let orders_opposite = match direction {
            MotorDir::Down => self.orders_up,
            _ => self.orders_down
        };

        let top_floor = N_FLOORS-1;

        let is_at_top_or_bottom: bool = match floor {
            top_floor   => true,
            0           => true,
            _           => false
        };

        if is_at_top_or_bottom && orders_opposite[floor] {
            return true;
        }

        return false;
    }


    /*pub fn cost_function(&self, floor: usize, direction: MotorDir, other_floors: [usize, ], other_directions: [usize, ]) {

    }*/

}
