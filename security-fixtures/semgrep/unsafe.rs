fn bad_unsafe_block(ptr: *const i32) {
    // ruleid: markymark.rust.unsafe-block
    unsafe {
        let _value = *ptr;
    }
}

fn good_no_unsafe() {
    // ok: markymark.rust.unsafe-block
    let _x = 1 + 1;
}

fn bad_transmute(x: u32) {
    // ruleid: markymark.rust.mem-transmute
    let _y: f32 = std::mem::transmute(x);
}

fn good_checked_conversion(x: u32) {
    // ok: markymark.rust.mem-transmute
    let _ = f32::from_bits(x);
}
