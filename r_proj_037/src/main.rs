use std::collections::btree_map::BTreeMap;
mod Foo;
/// some Rust collection examples
///
fn example1() {
    let mut count = BTreeMap::new();
    let message = "she sells sea shells by the sea shore";

    for c in message.chars() {
        *count.entry(c).or_insert(0) += 1;
    }

    println!("Number of each char in message");

    for (char, count) in &count {
        println!("{char}: {count}");
    }
}

fn example2() {
    struct Person { blood_alcohol: f32 }

    let orders = [1, 2, 1, 1, 2, 3, 4, 1, 2, 1, 4, 1, 1];

    let mut blood_alcohol = BTreeMap::new();

    for id in orders {

        let person = blood_alcohol.entry(id).or_insert(Person {blood_alcohol: 0.0});

        person.blood_alcohol *= 0.9;

        if person.blood_alcohol >= 0.3 {
            println!("Sorry {id}, you are totally drunk, you have a level of: {:?}", person.blood_alcohol);
        }
        else {
            person.blood_alcohol += 0.1;
        }
    }
}

fn example3() {
    let mut map = BTreeMap::new();

    map.insert(Foo::Foo {a: 1, b: "baz"}, 99);

    map.insert(Foo::Foo {a: 1, b: "xyz"}, 100);

    println!("v: {:?}", map.values().next().unwrap());
    println!("v: {:?}", map);
}

fn main() {
    println!("Hello, r_proj_037!");

    example1();
    example2();
    example3();
}
