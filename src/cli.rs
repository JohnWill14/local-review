use clap::Parser;
use std::path::PathBuf;
#[derive(Parser, Debug)]
#[command(
    name = "local-review",
    version,
    about = "Interactive local Git review",
    after_help = "INTERACTIVE SHORTCUTS:\n  Tab / Shift+Tab  move to the panel on the right / left\n  b / l             choose base / head\n  f                 search files or diff text\n  n / N             next / previous search result\n  g                 list commits; y copies the hash\n  v / V             select lines; c adds a comment\n  X                 export notes and snippets to the clipboard\n  t                 switch theme\n  h / H / ?         contextual / full help\n  q                 quit"
)]
pub struct Args {
    #[arg(
        short,
        long,
        value_name = "PATH",
        help = "Git repository (default: current directory)"
    )]
    pub path: Option<PathBuf>,
    #[arg(long, value_name = "REF")]
    pub base: Option<String>,
    #[arg(long, value_name = "REF")]
    pub head: Option<String>,
    #[arg(long, help = "Include unstaged/staged work when head is checked out")]
    pub working_tree: bool,
}
