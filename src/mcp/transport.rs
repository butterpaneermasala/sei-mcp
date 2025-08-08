use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub async fn run_loop<F>(mut handler: F) -> anyhow::Result<()>
where
    F: FnMut(String) -> Option<String> + Send + 'static,
{
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut writer = stdout;

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            break;
        }
        if let Some(resp) = handler(line) {
            writer.write_all(resp.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }
    }
    Ok(())
}
