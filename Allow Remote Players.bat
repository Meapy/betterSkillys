@ECHO OFF
REM Run as administrator (right-click > Run as administrator) once, to let other players connect.
REM Opens TCP 8080 (account server) and 2051 (world server) in Windows Firewall.
REM Your router must also forward TCP 8080 and 2051 to this PC.
NET SESSION >NUL 2>&1 || (ECHO Please right-click this file and choose "Run as administrator". & PAUSE & EXIT /B 1)
netsh advfirewall firewall delete rule name="betterSkillys (8080, 2051)" >NUL 2>&1
netsh advfirewall firewall add rule name="betterSkillys (8080, 2051)" dir=in action=allow protocol=TCP localport=8080,2051
PAUSE
