; HubDeck's additions to Tauri's NSIS installer (bundle > windows > nsis > installerHooks).

; HubDeck lists itself under Open with for .streamDeckPlugin and .streamDeckIconPack files at each
; start (src/assoc.rs);
; the uninstaller takes that out again, and the file's app only when it was HubDeck.
; An update runs the uninstaller too ($UpdateMode 1): the keys stay for the new version.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegKey HKCU "Software\Classes\HubDeck.streamDeckPlugin"
    DeleteRegValue HKCU "Software\Classes\.streamDeckPlugin\OpenWithProgids" "HubDeck.streamDeckPlugin"
    ReadRegStr $0 HKCU "Software\Classes\.streamDeckPlugin" ""
    ${If} $0 == "HubDeck.streamDeckPlugin"
      DeleteRegValue HKCU "Software\Classes\.streamDeckPlugin" ""
    ${EndIf}
    DeleteRegKey HKCU "Software\Classes\HubDeck.streamDeckIconPack"
    DeleteRegValue HKCU "Software\Classes\.streamDeckIconPack\OpenWithProgids" "HubDeck.streamDeckIconPack"
    ReadRegStr $0 HKCU "Software\Classes\.streamDeckIconPack" ""
    ${If} $0 == "HubDeck.streamDeckIconPack"
      DeleteRegValue HKCU "Software\Classes\.streamDeckIconPack" ""
    ${EndIf}
  ${EndIf}
!macroend
