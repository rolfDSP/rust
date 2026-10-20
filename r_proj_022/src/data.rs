pub struct Data {
    a: i32,
    b: i32,
}

impl Data {
    /// new
    ///
    pub fn new(a: i32, b: i32) -> Self {
        Data { a , b }
    }

    pub fn get_a(&self) -> i32 {
        self.a
    }

    pub fn get_b(&self) -> i32 {
        self.b
    }
}

