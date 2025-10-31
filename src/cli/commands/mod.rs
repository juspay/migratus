pub mod batch;
pub mod complete;
pub mod enrich;
pub mod load;
pub mod migrate;
pub mod status;
pub mod update;
pub mod validate;

// Re-export command handlers
pub use batch::handle_batch;
pub use complete::handle_complete;
pub use enrich::handle_enrich;
pub use load::handle_load;
pub use migrate::handle_migrate;
pub use status::handle_status;
pub use update::handle_update;
pub use validate::handle_validate;
