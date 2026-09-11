/// Plugin trait for future extensibility.
/// Not implemented in v1 -- prepared for:
/// - Claude Code integration
/// - Ticket management
/// - External connectors
/// - Integrated terminal
#[allow(dead_code)]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn init(&self) -> Result<(), String>;
}
