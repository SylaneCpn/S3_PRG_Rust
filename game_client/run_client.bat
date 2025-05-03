@echo off

if [%1]==[--release] (
  set opt=%1
  set target_dir=target\release
  set args=%2 %3 %4 %5 %6 %7 %8 %9
) else (
  set opt=
  set target_dir=target\debug
  set args=%*
)

set crate_name=game_client
set lib_name=%crate_name%.dll

set PATH=%PATH%;%target_dir%
cargo build %opt%
if %errorlevel%==0 (
  python NtvPy.py %crate_name% %args%
)

