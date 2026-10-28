use std::collections::HashMap;

fn logic(numbers: Vec<i32>) -> Vec<(i32, u32)> {
    let mut frequencies = HashMap::new();

    for num in numbers {
        let frequency = frequencies.entry(num).or_insert(0);
        *frequency += 1;
    }

    let mut result = Vec::new();

    for (num, frequency) in frequencies {
        result.push((num, frequency));
    }

    result
}

fn main() {
    println!("Hello, r_proj_035!");

    let numbers = vec![2,4,1,2,4,3,6,2,8,1];
    let result = logic(numbers);
    // print the result
    println!(
        "The frequency of each number is: {:?}",
        result
    )
}
