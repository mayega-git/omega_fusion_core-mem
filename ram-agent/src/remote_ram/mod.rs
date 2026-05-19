mod common;
mod server;
mod client;

pub use common::{RamExport, RamDistante, SHARED_RAM_PATH, REMOTE_RAM_PATH};
pub use server::{exporter, arreter};
pub use client::{connecter, deconnecter, chemin_mmap};
