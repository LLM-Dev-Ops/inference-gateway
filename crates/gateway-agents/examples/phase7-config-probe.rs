//! Native process probe for the real Phase7 startup configuration boundary.
//!
//! Inputs are supplied through each child process's own environment. Never run
//! this diagnostic with live credentials; it emits no raw Debug representation.

use gateway_agents::phase7::Phase7Config;

fn main() {
    let config = Phase7Config::from_env();
    println!(
        "{}",
        serde_json::json!({
            "accepted": true,
            "phase": config.agent_phase,
            "layer": config.agent_layer,
            "debug_redacted": !format!("{config:?}").contains(&config.ruvector_api_key),
        })
    );
}
