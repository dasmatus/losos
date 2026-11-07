//! Build-time embedded default page HTML (no runtime MDX parser).
//! Generated from `admin-ui/app/src/pages/*.mdx` via `losos-mdx` in `build.rs`.

include!(concat!(env!("OUT_DIR"), "/defaults.rs"));
