@ECHO OFF
REM Runs the client on this PC with the AIR runtime from tools\airsdk.
REM Rebuild after changing client\src: python tools\build_client.py
CD /D "%~dp0"
IF NOT EXIST "tools\airsdk\bin\adl.exe" (
    ECHO tools\airsdk not found. Unzip AIRSDK_Compiler.zip (AIR SDK 32^) into tools\airsdk.
    PAUSE
    EXIT /B 1
)
IF NOT EXIST "client\bin\client.swf" python tools\build_client.py || (PAUSE & EXIT /B 1)
START "" /D "client" "%~dp0tools\airsdk\bin\adl.exe" -nodebug WebMain-app.xml bin
