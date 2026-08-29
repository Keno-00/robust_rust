CREATE TABLE IF NOT EXISTS discord_interactions (
    interaction_id TEXT PRIMARY KEY,
    actor_hash TEXT NOT NULL,
    command_name TEXT NOT NULL,
    outcome TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_discord_interactions_created
    ON discord_interactions(created_at);

CREATE INDEX IF NOT EXISTS idx_discord_interactions_outcome_created
    ON discord_interactions(outcome, created_at);
