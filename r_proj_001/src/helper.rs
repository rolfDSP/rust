

pub fn fmt1(mut name: String, v: i32) -> String{
    name.push_str(&v.to_string());
    name
}
