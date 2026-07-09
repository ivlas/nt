mod atomic;
mod paths;

pub use atomic::atomic_write;
pub use paths::{absolute_path, index_path, nt_home, relative_to_cwd};
