/// This is a docstring for a standard function.
pub fn standard_function(x: i32) -> bool {
    x > 0
}

#[inline(always)]
pub async fn async_worker<T: std::fmt::Debug>(payload: T) {
    println!("{:?}", payload);
}

impl MyStruct {
    fn method_call(&self) {
        let x = vec![1, 2, 3];
        // This exercises ast::CallExpr
        standard_function(x.len() as i32);
        // This exercises ast::MethodCallExpr
        x.push(4);
    }
}
