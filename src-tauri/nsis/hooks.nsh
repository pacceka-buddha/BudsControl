; Extra steps for the NSIS installer (see bundle.windows.nsis.installerHooks).

!macro NSIS_HOOK_POSTUNINSTALL
  ; "Launch at login" lives in HKCU\...\Run (created by the app, not by this installer): remove it so
  ; nothing is left behind and Windows doesn't try to start a program that no longer exists.
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Buds Control"
!macroend
