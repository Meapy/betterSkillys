@ECHO OFF
REM Starts Redis (the database, port 6379, this PC only), the account server (port 8080)
REM and the world server (port 2051), each in its own window. Close those windows to stop.
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
TASKLIST /FI "IMAGENAME eq WorldServer.exe" /NH | FINDSTR /B /I "WorldServer.exe" >NUL || (
    ECHO Starting the world server...
    START "World Server" /D "WorldServer\bin\Debug\net8.0" WorldServer.exe
)
