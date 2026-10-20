mod data;

fn main() {
    println!("Hello, r_proj_022!");

    let d = data::Data::new (10, 20);

    println!("data.a = {}, data.b = {}", d.get_a(), d.get_b());

    let vd = vec![data::Data::new(0,0), data::Data::new(2,3), data::Data::new(100,-100)];
    for d in &vd {
        println!("a = {}, b = {}", d.get_a(), d.get_b());
    }

    let mut v2: Vec<data::Data> = Vec::new();
    v2.push(data::Data::new(10, 30));
    v2.push(data::Data::new(-5, 4));
    v2.push(data::Data::new(0,0));

    for d in &v2 {
        println!("a = {}, b = {}", d.get_a(), d.get_b());
    }

    let v3: Vec<data::Data> = (0..5).map(|i| data::Data::new(i, i)).collect();
    for d in &v3 {
        println!("a = {}, b = {}", d.get_a(), d.get_b());
    }

}
