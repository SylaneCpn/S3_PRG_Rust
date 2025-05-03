use std::env::args;

fn main() {
    string_and_args();
}


pub fn count_and_compute() {
    for i in 0..20 {
        let value = i as f32 * 0.1 * std::f32::consts::PI;
        let sin = value.sin();
        println!("counter={i}\tvalue={value}\tsin(value)={sin}");
    }
}

pub fn store_and_change() {
    let mut v: Vec<_> = (0..20)
        .map(|x| (x as f32 * 0.1 * std::f32::consts::PI).sin())
        .collect();
    dbg!(&v);
    v.resize(v.len() * 2, 0.0);
    v.iter_mut().enumerate().for_each(|(i, val)| {
        *val += (i as f32 * 0.2 * std::f32::consts::PI).sin();
    });
    v.iter().enumerate().for_each(|(i, e)| {
        println!("{}: {}", i, e);
    });
}

pub fn string_and_args() {
    let a = args();
    let mut txt = Vec::new();
    let mut integer: i32 = 0;
    for arg in a {
        println!("arg: \"{arg}\"");
        match arg.parse::<i32>() {
            Ok(val) => {
                integer += val;
            }
            Err(_) => {
                txt.push(arg.to_string());
            }
        }
    }
    println!("integer: {integer}");
    println!("text: {}", txt.join("|"));
}