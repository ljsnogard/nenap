#![no_std]
// denco 的 `TrEncodable` / `TrDecodable` 派生用 GAT + `impl Trait` 表示异步操作。
#![feature(impl_trait_in_assoc_type)]

pub mod v1;

pub mod x_deps {
    pub use abs_async_iter;
    pub use abs_cancel;
    pub use abs_str;
    pub use anylr;

    pub use abs_denco;
    pub use denco_macro;

    pub use mm_ptr;
}
