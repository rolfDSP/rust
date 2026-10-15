use half::f16;

fn main() {
    println!("Hello, world!");

    // Variable shadowing
    let x = 32.0;
    println!("x= {}", x);

    let x = "thirty two";
    println!("x= {}", x);

    {
        let x = 16.0;

        println!("x= {}", x);
    }
    println!("x= {}", x);

    let r1 = Some(42);
    println!("r1= {:?}", r1);

    fn div(x1: f16, x2: f16) -> Option<f16> {
        // if x2 != 0, then return something valid, otherwise make the variable invalid
        if x2 == f16::ZERO {
            None
        }
        else {
            Some(x1 / x2)
        }
    }

    let m1 = div(f16::from_f32(1.0), f16::from_f32(0.0));
    println!("m1: {:?}", m1);
    match  m1 {
        Some(value) => println!("m1 is valid: {}", value),
        None => println!("m1 is invalid: division by zero"),
    }

    let mut r3 = None;
    let b1 = true;
    if b1 {
        r3 = Some(100);
    }
    println!("r3= {:?}", r3);

    let v1 = match r3 {
        Some(v) => v,
        None => 0
    };

    println!("r3 = {}", v1);

    let Some(value) = r3 else {
        println!("Value is None");
        return;
    };

    println!("value= {}", value);

    if let Some(vv) = r3 {
        println!("vv={}", vv);
    }
}
