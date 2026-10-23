// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Claude Code hooks run this program with --claude-hook: report and exit, no window
    if std::env::args().nth(1).as_deref() == Some("--claude-hook") {
        return hubdeck_lib::claude_hook();
    }
    // an AI assistant (Claude Desktop, Claude Code...) starts it with --mcp: serve on stdio, no window
    if std::env::args().nth(1).as_deref() == Some("--mcp") {
        return hubdeck_lib::mcp();
    }
    // the same, with the deck's own tools (switched on per project)
    if std::env::args().nth(1).as_deref() == Some("--mcp-deck") {
        return hubdeck_lib::mcp_deck();
    }
    // HubDeck's own deck assistant (its Claude Code session): drafts only
    if std::env::args().nth(1).as_deref() == Some("--mcp-assistant") {
        return hubdeck_lib::mcp_assistant();
    }
    // the guard HubDeck starts for itself: hands the deck back if HubDeck dies (guard.rs)
    if std::env::args().nth(1).as_deref() == Some("--guard") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        return hubdeck_lib::guard(&args);
    }
    // the release gate: what Install would get for the most popular catalog plugins here
    if std::env::args().nth(1).as_deref() == Some("--catalog-check") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        std::process::exit(hubdeck_lib::catalog_check(&args));
    }
    // a terminal or a script: `hubdeck cli status`, `hubdeck cli profile Desk`...
    if std::env::args().nth(1).as_deref() == Some("cli") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        std::process::exit(hubdeck_lib::cli(&args));
    }
    hubdeck_lib::run()
}
