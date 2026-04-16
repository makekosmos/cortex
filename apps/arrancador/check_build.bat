@echo off
echo Checking Electron app compilation with Bun + Vite 8...
call bun run typecheck
if %errorlevel% neq 0 (
    echo [ERROR] Frontend TypeScript errors found!
    pause
    exit /b %errorlevel%
) else (
    echo [SUCCESS] Frontend TypeScript checks passed.
)

echo.
echo Checking Electron build pipeline...
call bun run build
if %errorlevel% neq 0 (
    echo [ERROR] Electron build failed!
    pause
    exit /b %errorlevel%
) else (
    echo [SUCCESS] Electron renderer build passed.
)

echo.
echo Checking Rust sidecar compilation...
cd src-tauri
cargo check
if %errorlevel% neq 0 (
    echo [ERROR] Rust sidecar compilation failed!
    pause
    exit /b %errorlevel%
) else (
    echo [SUCCESS] Rust sidecar compiles successfully.
)

echo.
echo All checks passed! Bun toolchain, Electron shell and Rust sidecar are compiling.
pause
