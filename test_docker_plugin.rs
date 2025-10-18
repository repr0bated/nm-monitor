use nm_monitor::state::plugins::DockerStatePlugin;
use nm_monitor::state::plugin::StatePlugin;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Docker plugin...");

    let plugin = DockerStatePlugin::new();

    match plugin.query_current_state().await {
        Ok(state) => {
            println!("Docker state query successful!");
            println!("{}", serde_json::to_string_pretty(&state)?);
        }
        Err(e) => {
            println!("Docker state query failed: {}", e);
            // This is expected if Docker is not available
        }
    }

    Ok(())
}
