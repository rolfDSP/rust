pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

pub fn add_num(mut s: String, b: i32) -> String{
    s.push_str(&b.to_string());
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    fn test_add_num() {
        let result = add_num("Test".to_string(), 42);
        assert_eq!(result, "Test42");
    }
}
