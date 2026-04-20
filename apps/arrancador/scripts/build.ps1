$ErrorActionPreference = "Stop"

bun run predev
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

bun run build:renderer
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

bun run build:main
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

bun run build:preload
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
