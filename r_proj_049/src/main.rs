use std::collections::HashMap;

fn main() {
    println!("Hello, r_proj_039!");

    let mut mp : HashMap<String, i32> = HashMap::new();

    mp.insert("Hallo".to_string(), 1);
    mp.insert("Welt".to_string(), 1);
    mp.entry("Total".to_string()).or_insert(1);

    if let Some(v) = mp.get_mut("Hallo") {
        *v += 1;
    }

    mp.entry("Spass".to_string()).and_modify(|v| *v += 1).or_insert(1);
    mp.entry("Spass".to_string()).and_modify(|v| *v += 1).or_insert(1);

    println!("{:?}", mp);

}
