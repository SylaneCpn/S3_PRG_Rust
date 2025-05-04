use std::ffi::*;

#[no_mangle]
fn say_hello() {
    println!("Hello from Rust!");
}

#[no_mangle]
fn compute(first: c_double, second: c_double, op: *const c_char) -> c_double {
    let operation = unsafe { CStr::from_ptr(op) }.to_string_lossy();

    match operation.as_ref() {
        "add" => first + second,
        "sub" => first - second,
        "mul" => first * second,
        "div" => first / second,
        _ => 0.0,
    }
}

#[no_mangle]
fn transform(data: *mut c_double, len: usize) {
    let values = unsafe { std::slice::from_raw_parts_mut(data, len) };
    values.reverse();
    values.iter_mut().for_each(|x| {
        *x += 10.0;
    });
}
