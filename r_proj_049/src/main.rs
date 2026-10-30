use std::collections::HashMap;
use std::fs;
use std::io;

fn read_file(f_name: String) -> Result<Vec<String>, io::Error>{
    // open a text file with the name f_name and
    // read the complete text into a vector
    // of lines.
    // in case of an error return the error state
    let text = fs::read_to_string(&f_name)?;
    let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    Ok(lines)
}

fn main() {
    println!("Hello, r_proj_039!");

    // let mut mp : HashMap<String, i32> = HashMap::new();
    //
    // mp.insert("Hallo".to_string(), 1);
    // mp.insert("Welt".to_string(), 1);
    // mp.entry("Total".to_string()).or_insert(1);
    //
    // if let Some(v) = mp.get_mut("Hallo") {
    //     *v += 1;
    // }
    //
    // mp.entry("Spass".to_string()).and_modify(|v| *v += 1).or_insert(1);
    // mp.entry("Spass".to_string()).and_modify(|v| *v += 1).or_insert(1);
    //
    // println!("{:?}", mp);

    let v1 = read_file("mytext.txt".to_string()).unwrap();
    //println!("{:?}", v1);
    println!("length: {}", v1.len());

    let mut mp : HashMap<String, i32> = HashMap::new();

    for lines in v1 {
        println!("Line: {}", lines);
        let b1: Vec<&str> = lines.split(" ").collect();
        println!("{:?}", b1.len());
        for word in b1 {
            mp.entry(word.to_string()).and_modify(|v| *v += 1).or_insert(1);
        }
    }
    println!("{:?}", mp);

    let mut w1_list: Vec<String> = Vec::new();

    for (key, value) in &mp {
        if *value>1 { w1_list.push(key.to_string())};
    }
    println!("{:?}", w1_list);

    let mut max_key: String = "".to_string();
    let mut max_val: i32 = 1;
    for (key, value) in &mp {
        if *value > max_val { max_val = *value; max_key = key.to_string();}
    }

    println!("Most often used word: {}, was used {} times", max_key, max_val);
}
