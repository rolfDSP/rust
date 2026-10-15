use half::f16;

// function : y = add(x1, x2)
// Adds to numbers an return the result
// x1, x2: Input arguments, float16 numbers
// y: Return value, float16 number
fn add(x1: f16, x2: f16) -> f16 {
    x1 + x2
}

// function: y = avg2(x1, x2)
// Take the average of the two argument numbers and return the result
// x1, x2: Input arguments, float 16
// y: Result, float 16
fn avg2(x1: f16, x2: f16) -> f16 {
    // Compute in f32 so the intermediate sum cannot overflow f16
    f16::from_f32((x1.to_f32() + x2.to_f32()) / 2.0)
}

// function: y = div(x1, x2)
// Divides x1 by x2. Example for Option<T>: a division by zero has no
// result, so the function returns None instead of a number.
// x1, x2: Input arguments, float16 numbers
// y: Some(x1 / x2) if x2 is not zero, otherwise None
fn div(x1: f16, x2: f16) -> Option<f16> {
    if x2 == f16::ZERO {
        None
    } else {
        Some(x1 / x2)
    }
}

fn main() {
    let x1 = f16::from_f32(1.5);
    let x2 = f16::from_f32(2.25);
    let y = add(x1, x2);
    println!("{} + {} = {}", x1, x2, y);
    let avg = avg2(x1, x2);
    println!("avg2({}, {}) = {}", x1, x2, avg);

    // Option<T> example: div() returns either Some(value) or None
    for divisor in [x2, f16::ZERO] {
        // 1. match: handle both cases explicitly
        match div(x1, divisor) {
            Some(q) => println!("div({}, {}) = Some({})", x1, divisor, q),
            None => println!("div({}, {}) = None (division by zero)", x1, divisor),
        }

        // 2. if let: only act when there is a value
        if let Some(q) = div(x1, divisor) {
            println!("  if let: got the value {}", q);
        }

        // 3. unwrap_or: use a fallback value for None
        let q = div(x1, divisor).unwrap_or(f16::ZERO);
        println!("  unwrap_or(0): {}", q);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let x1 = f16::from_f32(1.5);
        let x2 = f16::from_f32(2.25);
        assert_eq!(add(x1, x2), f16::from_f32(3.75));
    }

    #[test]
    fn test_avg2() {
        let x1 = f16::from_f32(1.5);
        let x2 = f16::from_f32(2.25);
        assert_eq!(avg2(x1, x2), f16::from_f32(1.875));
    }

    #[test]
    fn test_div() {
        let x1 = f16::from_f32(3.0);
        let x2 = f16::from_f32(2.0);
        assert_eq!(div(x1, x2), Some(f16::from_f32(1.5)));
    }

    #[test]
    fn test_div_by_zero() {
        assert_eq!(div(f16::ONE, f16::ZERO), None);
    }

    #[test]
    fn test_avg2_no_overflow() {
        assert_eq!(avg2(f16::MAX, f16::MAX), f16::MAX);
    }
}
