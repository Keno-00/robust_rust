const applicationId = process.env.DISCORD_APPLICATION_ID?.trim();
const token = process.env.DISCORD_BOT_TOKEN?.trim();

if (!applicationId || !/^\d+$/.test(applicationId)) {
  throw new Error("DISCORD_APPLICATION_ID is missing or invalid.");
}
if (!token) throw new Error("DISCORD_BOT_TOKEN is missing.");

const commands = [
  { name: "service", description: "Show license-service and database health" },
  { name: "security", description: "Show security event counts for 1h, 24h, and 7d" },
  { name: "audit", description: "Show recent privacy-safe license audit events" },
  { name: "licenses", description: "Show aggregate license counts and usage" },
  {
    name: "license",
    description: "Inspect one customer license without exposing its key",
    options: [{
      type: 3,
      name: "customer",
      description: "At least 3 characters of the customer name or email",
      required: true,
      min_length: 3,
      max_length: 100,
    }],
  },
  { name: "sessions", description: "Show active licensed application sessions" },
  { name: "version", description: "Show active client versions and the required minimum" },
];

const response = await fetch(`https://discord.com/api/v10/applications/${applicationId}/commands`, {
  method: "PUT",
  headers: {
    Authorization: `Bot ${token}`,
    "Content-Type": "application/json",
  },
  body: JSON.stringify(commands),
});

if (!response.ok) {
  const message = await response.text();
  throw new Error(`Discord command registration failed (HTTP ${response.status}): ${message}`);
}

const registered = await response.json();
console.log(`Registered ${registered.length} global commands: ${registered.map((command) => command.name).join(", ")}`);
