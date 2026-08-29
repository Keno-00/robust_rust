$ErrorActionPreference = "Stop"

$secureToken = Read-Host "Enter the reset Discord bot token" -AsSecureString
$tokenPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secureToken)

try {
    $env:DISCORD_APPLICATION_ID = "1194649064181088376"
    $env:DISCORD_BOT_TOKEN = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($tokenPointer)
    npm run discord:register
    if ($LASTEXITCODE -ne 0) {
        throw "Discord command registration failed with exit code $LASTEXITCODE."
    }
}
finally {
    Remove-Item Env:DISCORD_BOT_TOKEN -ErrorAction SilentlyContinue
    Remove-Item Env:DISCORD_APPLICATION_ID -ErrorAction SilentlyContinue
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($tokenPointer)
    $secureToken.Dispose()
}
