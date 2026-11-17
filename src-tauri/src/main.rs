#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if liubai_lib::source_io::worker_entry_if_requested() { return; }
    if liubai_lib::mcp::stdio_entry_if_requested() { return; }
    liubai_lib::run();
}
