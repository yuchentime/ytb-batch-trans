@echo off
rem Windows local dev launcher: run Tauri dev in a minimal PATH + vcvars64.
rem
rem Why: the machine's normal PATH can contain JDK api-ms-win-*.dll shims and
rem Git's link.exe, which break MSVC linking and make linked binaries exit with
rem STATUS_ENTRYPOINT_NOT_FOUND. This wrapper keeps only the system directories
rem plus cargo/node/python/ffmpeg (discovered from the current PATH first), then
rem pulls in the MSVC environment, so `npm run tauri dev` finds cargo and the
rem whisper/ffmpeg CLIs.
rem
rem Usage (from the repo root):
rem   scripts\dev.cmd npm run tauri dev
setlocal enableextensions

for %%i in (node.exe) do set "OVD_NODE=%%~dp$PATH:i"
for %%i in (python.exe) do set "OVD_PYTHON=%%~dp$PATH:i"
for %%i in (ffmpeg.exe) do set "OVD_FFMPEG=%%~dp$PATH:i"

set "PATH=C:\Windows\System32;C:\Windows;C:\Windows\System32\Wbem;C:\Windows\System32\WindowsPowerShell\v1.0;C:\Windows\System32\OpenSSH;%USERPROFILE%\.cargo\bin;C:\Program Files\Git\cmd"
if defined OVD_NODE set "PATH=%PATH%;%OVD_NODE%"
if defined OVD_PYTHON set "PATH=%PATH%;%OVD_PYTHON%;%OVD_PYTHON%Scripts"
if defined OVD_FFMPEG set "PATH=%PATH%;%OVD_FFMPEG%"

set "OVD_VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if exist "%OVD_VSWHERE%" (
  for /f "usebackq tokens=*" %%i in (`"%OVD_VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "OVD_VS=%%i"
)
if defined OVD_VS (
  call "%OVD_VS%\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
) else (
  call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
)

%*
endlocal
