use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};

pub struct DiscordSession {
    client: DiscordIpcClient,
}

pub fn start(
    client_id: &str,
    state: &str,
    details: &str,
    start_ts: i64,
) -> Result<DiscordSession, String> {
    try_start(client_id, state, details, start_ts).map_err(|e| e.to_string())
}

fn try_start(
    client_id: &str,
    state: &str,
    details: &str,
    start_ts: i64,
) -> Result<DiscordSession, Box<dyn std::error::Error>> {
    // new() only allocates the struct and cannot fail
    let mut client = DiscordIpcClient::new(client_id);
    client.connect()?;
    client.set_activity(
        activity::Activity::new()
            .state(state)
            .details(details)
            .timestamps(activity::Timestamps::new().start(start_ts)),
    )?;
    Ok(DiscordSession { client })
}

impl Drop for DiscordSession {
    fn drop(&mut self) {
        self.client.clear_activity().ok();
        self.client.close().ok();
    }
}
