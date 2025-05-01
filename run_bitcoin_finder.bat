@echo off
echo Running Bitcoin Private Key Finder (Rust)
echo.
echo Building with warnings enabled...
cargo rustc --release -- -W warnings

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Compilation failed with warnings. Fix them before running.
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo Starting application...
echo.
.\target\release\btcrustai.exe

pause 