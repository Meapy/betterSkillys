@ECHO OFF
REM Starts Redis (the database, port 6379, this PC only), the account server (port 8080)
REM and the world server (port 2051, behind the world proxy), each in its own window. Close those windows to stop.
REM Build the Rust client for players (dist\betterSkillys-rs-windows.zip, connecting to the public
REM address) and publish it to dist\client, where players' clients update from; quick when nothing
REM changed. A failed build doesn't stop the servers.
python "%~dp0tools\build_client_rs.py" --account http://188.141.12.3:8080 --package --publish || ECHO The client build failed; starting the servers anyway.
CD /D "%~dp0source"
REM Rebuild first (quick when nothing changed) unless a server is already running.
TASKLIST /FI "IMAGENAME eq App.exe" /NH | FINDSTR /B /I "App.exe" >NUL && GOTO START
TASKLIST /FI "IMAGENAME eq WorldServer.exe" /NH | FINDSTR /B /I "WorldServer.exe" >NUL && GOTO START
dotnet build betterSkillys.sln -c Debug -v q --nologo || (PAUSE & EXIT /B 1)
:START
TASKLIST /FI "IMAGENAME eq redis-server.exe" /NH | FINDSTR /B /I "redis-server.exe" >NUL || (
    ECHO Starting Redis...
    START "Redis" /D "Redis" redis-server.exe redis.local.conf
    TIMEOUT /T 2 /NOBREAK >NUL
)
TASKLIST /FI "IMAGENAME eq App.exe" /NH | FINDSTR /B /I "App.exe" >NUL || (
    ECHO Starting the account server...
    START "Account Server" /D "App\bin\Debug\net8.0" App.exe
)
REM The world proxy owns port 2051 and forwards players to a world server in build\live\a or b,
REM so "python tools\rolling_restart.py deploy" updates the world server without kicking anyone.
REM If they aren't running this starts them; if they are, it builds the current source and
REM switches to it without kicking anyone (players move over at their next trip to the Nexus).
CD ..
python tools\rolling_restart.py update || ECHO The world server didn't start or update; see the message above.
