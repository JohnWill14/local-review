use anyhow::Result;
use clap::Parser;
use local_review::{
    app::App, cli::Args, config::Config, git::RepositoryModel, notes::NoteStore, ui::run,
};

fn main() -> Result<()> {
    let args = Args::parse();
    let config = Config::load()?;
    let repo = RepositoryModel::open(args.path.as_deref())?;
    let base = args
        .base
        .or_else(|| repo.default_base())
        .unwrap_or_else(|| "HEAD~1".into());
    let head = args
        .head
        .unwrap_or_else(|| repo.current_ref().unwrap_or_else(|| "HEAD".into()));
    let notes = NoteStore::open(&Config::data_dir()?.join("notes.sqlite"))?;
    run(App::new(
        repo,
        notes,
        base,
        head,
        args.working_tree,
        config.theme,
    )?)
}
