$ErrorActionPreference = "Stop"

bun run build:sidecar
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

bun run prepare:native
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

bun run build:renderer
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

bun run build:main
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

bun run build:preload
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
