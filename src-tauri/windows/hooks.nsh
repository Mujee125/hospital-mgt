; HMS server-build post-install hook.
;
; Runs once, automatically, with the Administrator privileges the NSIS
; installer already has (installMode is perMachine — see tauri.server.conf.json).
; This is the ONLY place PostgreSQL provisioning happens. The HMS app itself
; never requests elevation and never needs to.
;
; Steps:
;   1. Create C:\ProgramData\HMS and grant standard users write access to it
;      (so the receptionist's normal, non-admin login can save settings like
;      clinic name / WhatsApp group later — see config.rs).
;   2. Run initdb to create a fresh PostgreSQL data cluster with a randomly
;      generated password, UNLESS a cluster already exists (re-install /
;      upgrade safety — never destroys existing patient data).
;   3. Lock down pg_hba.conf to LAN-only, scram-sha-256 auth.
;   4. Register PostgreSQL as a native Windows Service set to auto-start.
;   5. Start the service and write the generated credentials + server
;      config into C:\ProgramData\HMS\config.json.
;
; If anything in here fails, the installer shows a clear message rather
; than silently producing a broken install — better to fail loudly at
; install time than mysteriously at every future app launch.

!include "LogicLib.nsh"

!macro NSIS_HOOK_POSTINSTALL

  ; Ensure $APPDATA resolves to the ALL USERS location (C:\ProgramData),
  ; not the installing user's personal C:\Users\<admin>\AppData\Roaming.
  ; NSIS has no $PROGRAMDATA constant — this is the documented way to get
  ; that path. Tauri's own perMachine template likely already sets this,
  ; but we set it explicitly so this hook never depends on that assumption.
  SetShellVarContext all

  DetailPrint "Setting up HMS shared data folder..."
  CreateDirectory "$APPDATA\HMS"
  ; Explicitly grant standard users modify rights on this one folder, rather
  ; than relying on default ProgramData ACL inheritance across Windows
  ; versions. (M) = modify; applies to this folder, subfolders and files.
  nsExec::ExecToLog 'icacls "$APPDATA\HMS" /grant *S-1-5-32-545:(OI)(CI)M /T'



  ; Check if database is already provisioned AND config exists
  IfFileExists "$APPDATA\HMS\pgdata\PG_VERSION" 0 run_setup_init
  IfFileExists "$APPDATA\HMS\config.json" pg_already_provisioned run_setup_repair

run_setup_init:
  DetailPrint "Installing PostgreSQL binaries..."
  CreateDirectory "$APPDATA\HMS\pgsql"
  ; Use robocopy instead of NSIS CopyFiles:
  ;   1. NSIS CopyFiles fails on long paths (>260 chars) like pgAdmin 4 docs.
  ;   2. robocopy handles long paths natively and lets us EXCLUDE the
  ;      pgAdmin 4 / docs / include folders we don't need (~200 MB savings).
  ;   3. /E = copy subdirs including empty, /NFL /NDL = no file/dir listing,
  ;      /NJH /NJS = no job header/summary, /XD = exclude directories.
  nsExec::ExecToLog 'robocopy "$INSTDIR\pgsql" "$APPDATA\HMS\pgsql" /E /NFL /NDL /NJH /NJS /XD "pgAdmin 4" "docs" "include" "symbols"'
  Pop $0
  ; robocopy exit codes: 0-7 are success (<8 = OK), >=8 = error.
  ${If} $0 >= 8
    MessageBox MB_OK|MB_ICONSTOP "Failed to copy PostgreSQL binaries (robocopy error $0). Setup cannot continue."
    Abort
  ${EndIf}
  Goto run_setup_common

run_setup_repair:
  DetailPrint "Repairing setup: resetting database credentials..."
  DetailPrint "Installing PostgreSQL binaries..."
  CreateDirectory "$APPDATA\HMS\pgsql"
  nsExec::ExecToLog 'robocopy "$INSTDIR\pgsql" "$APPDATA\HMS\pgsql" /E /NFL /NDL /NJH /NJS /XD "pgAdmin 4" "docs" "include" "symbols"'
  Pop $0
  ${If} $0 >= 8
    MessageBox MB_OK|MB_ICONSTOP "Failed to copy PostgreSQL binaries (robocopy error $0). Setup cannot continue."
    Abort
  ${EndIf}

  ; Stop service to overwrite pg_hba.conf safely
  nsExec::ExecToLog 'sc stop HMS-PostgreSQL'
  Sleep 2000

run_setup_common:
  DetailPrint "Generating database credentials..."
  FileOpen $6 "$APPDATA\HMS\gen_pw.ps1" w
  FileWrite $6 '$$bytes = New-Object byte[] 24$\r$\n'
  FileWrite $6 '$$rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()$\r$\n'
  FileWrite $6 '$$rng.GetBytes($$bytes)$\r$\n'
  FileWrite $6 '$$b64 = [Convert]::ToBase64String($$bytes)$\r$\n'
  FileWrite $6 '$$clean = ($$b64 -replace "[^a-zA-Z0-9]", "")$\r$\n'
  FileWrite $6 'Set-Content -Path "$APPDATA\HMS\pg_pwfile.tmp" -Value $$clean -NoNewline$\r$\n'
  FileClose $6

  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$APPDATA\HMS\gen_pw.ps1"'
  Pop $0
  Delete "$APPDATA\HMS\gen_pw.ps1"

  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONSTOP "Failed to generate a secure database password. Setup cannot continue."
    Abort
  ${EndIf}

  IfFileExists "$APPDATA\HMS\pg_pwfile.tmp" 0 pw_generation_failed
  Goto pw_generation_ok

pw_generation_failed:
  MessageBox MB_OK|MB_ICONSTOP "Failed to generate a secure database password. Setup cannot continue."
  Abort

pw_generation_ok:
  FileOpen $7 "$APPDATA\HMS\pg_pwfile.tmp" r
  FileRead $7 $1
  FileClose $7

  ; If database is already initialized, skip initdb and pg_ctl register
  IfFileExists "$APPDATA\HMS\pgdata\PG_VERSION" skip_initdb_and_register 0

  nsExec::ExecToLog '"$APPDATA\HMS\pgsql\bin\initdb.exe" -D "$APPDATA\HMS\pgdata" -U postgres --auth=trust --encoding=UTF8'
  Pop $0
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONSTOP "PostgreSQL initialization (initdb) failed. Setup cannot continue. Check that no previous PostgreSQL installation is using the same data directory."
    Abort
  ${EndIf}

  DetailPrint "Registering PostgreSQL as a Windows Service..."
  nsExec::ExecToLog '"$APPDATA\HMS\pgsql\bin\pg_ctl.exe" register -N "HMS-PostgreSQL" -D "$APPDATA\HMS\pgdata" -S auto -o "-p 5432" -w'
  Pop $0
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONSTOP "Failed to register the PostgreSQL Windows Service. Setup cannot continue. You may need to run this installer as Administrator."
    Abort
  ${EndIf}

skip_initdb_and_register:
  ; To set/reset password, we temporarily overwrite pg_hba.conf with local trust rules
  FileOpen $4 "$APPDATA\HMS\pgdata\pg_hba.conf" w
  FileWrite $4 "# Temporary trust rules for password configuration$\r$\n"
  FileWrite $4 "host    all             all             127.0.0.1/32            trust$\r$\n"
  FileWrite $4 "host    all             all             ::1/128                 trust$\r$\n"
  FileClose $4

  DetailPrint "Starting PostgreSQL service..."
  nsExec::ExecToLog 'sc start HMS-PostgreSQL'
  Sleep 2000 ; give the service a moment to come fully online before connecting

  DetailPrint "Securing database credentials..."
  FileOpen $8 "$APPDATA\HMS\set_pw.sql" w
  FileWrite $8 "ALTER USER postgres WITH PASSWORD '$1';$\r$\n"
  FileClose $8

  nsExec::ExecToLog '"$APPDATA\HMS\pgsql\bin\psql.exe" -U postgres -d postgres -f "$APPDATA\HMS\set_pw.sql"'
  Pop $0
  Delete "$APPDATA\HMS\set_pw.sql"
  Delete "$APPDATA\HMS\pg_pwfile.tmp"

  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONSTOP "Failed to set the database password. Setup cannot continue."
    Abort
  ${EndIf}

  ; ── NOW lock down network exposure: switch from trust to LAN-only,
  ;    encrypted-password auth. Must happen AFTER the password is set.
  ;    NOTE: SSL/TLS for the Postgres connection itself is enabled in a
  ;    SECOND phase, by the Rust app on its first launch (see lib.rs /
  ;    tls_provision.rs), not here. The self-signed certificate is
  ;    generated by the well-tested rcgen crate in Rust — reproducing that
  ;    logic in PowerShell here would be a second, divergent
  ;    implementation of the same thing. This hook gets scram-sha-256
  ;    auth + LAN scoping in place now; the app layers SSL on top of that
  ;    moments later, before ever exposing credentials to a client. ──
  DetailPrint "Configuring PostgreSQL security settings..."
  FileOpen $3 "$APPDATA\HMS\pgdata\postgresql.conf" a
  FileSeek $3 0 END
  FileWrite $3 "$\r$\nlisten_addresses = '*'$\r$\nport = 5432$\r$\nidle_in_transaction_session_timeout = 300000$\r$\n"
  FileClose $3

  FileOpen $4 "$APPDATA\HMS\pgdata\pg_hba.conf" w
  FileWrite $4 "# HMS managed rules — local app + LAN clients only, scram-sha-256 required.$\r$\n"
  FileWrite $4 "# Local loopback uses 'host' (plaintext) so the app can bootstrap on first$\r$\n"
  FileWrite $4 "# launch BEFORE SSL is provisioned. The Rust app upgrades this to 'hostssl'$\r$\n"
  FileWrite $4 "# once TLS is configured (see pg_provision.rs).$\r$\n"
  FileWrite $4 "host    all             all             127.0.0.1/32            scram-sha-256$\r$\n"
  FileWrite $4 "host    all             all             ::1/128                 scram-sha-256$\r$\n"
  FileWrite $4 "# LAN clients MUST use SSL — never allow plaintext over the network.$\r$\n"
  FileWrite $4 "hostssl all             all             10.0.0.0/8              scram-sha-256$\r$\n"
  FileWrite $4 "hostssl all             all             172.16.0.0/12           scram-sha-256$\r$\n"
  FileWrite $4 "hostssl all             all             192.168.0.0/16          scram-sha-256$\r$\n"
  FileClose $4

  DetailPrint "Restarting PostgreSQL to apply security settings..."
  nsExec::ExecToLog 'sc stop HMS-PostgreSQL'
  Sleep 2000
  nsExec::ExecToLog 'sc start HMS-PostgreSQL'
  Sleep 2000

  ; ── Write the machine-wide config with the generated credentials ──
  DetailPrint "Saving configuration..."
  FileOpen $5 "$APPDATA\HMS\config.json" w
  FileWrite $5 '{$\r$\n'
  FileWrite $5 '  "mode": "server",$\r$\n'
  FileWrite $5 '  "db_host": "127.0.0.1",$\r$\n'
  FileWrite $5 '  "db_port": 5432,$\r$\n'
  FileWrite $5 '  "db_user": "postgres",$\r$\n'
  FileWrite $5 '  "db_password": "$1",$\r$\n'
  FileWrite $5 '  "db_name": "hospital_db",$\r$\n'
  FileWrite $5 '  "clinic_name": "VitalFlow Clinic",$\r$\n'
  FileWrite $5 '  "doctors_whatsapp_group": "",$\r$\n'
  FileWrite $5 '  "setup_complete": true$\r$\n'
  FileWrite $5 '}$\r$\n'
  FileClose $5

  DetailPrint "PostgreSQL setup complete."
  Goto pg_setup_done

  pg_already_provisioned:
    DetailPrint "PostgreSQL already provisioned — verifying service is running..."
    nsExec::ExecToLog 'sc start HMS-PostgreSQL'

  pg_setup_done:

  ; Always (re-)apply firewall rules, on EVERY install/repair path — not
  ; just the fresh-install branch. Previously the 42011 rule lived only
  ; inside run_setup_common, so a repair/repeat install (pg_already_provisioned)
  ; skipped it entirely. The 5432 rule was missing altogether — PostgreSQL
  ; listens on all interfaces (listen_addresses='*' above) but the OS
  ; firewall still has to be told to let inbound LAN traffic through to it.
  ; Delete-then-add keeps this idempotent — "delete" is a harmless no-op
  ; if the rule doesn't exist yet — so re-running the installer never
  ; produces duplicate or stale rules.
  DetailPrint "Configuring Windows Firewall for pairing listener..."
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="HMS Pairing Port"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="HMS Pairing Port" dir=in action=allow protocol=tcp localport=42011 remoteip=192.168.0.0/16,10.0.0.0/8,172.16.0.0/12 enable=yes'

  DetailPrint "Configuring Windows Firewall for PostgreSQL..."
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="HMS PostgreSQL Port"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="HMS PostgreSQL Port" dir=in action=allow protocol=tcp localport=5432 remoteip=192.168.0.0/16,10.0.0.0/8,172.16.0.0/12 enable=yes'

  ; UDP discovery broadcast port (see discovery::DISCOVERY_PORT / lib.rs).
  ; Without this, clients can pair manually by typed IP but the automatic
  ; "find the server on the LAN" fallback (used at normal client startup
  ; when the saved IP stops responding) silently fails too.
  DetailPrint "Configuring Windows Firewall for LAN discovery..."
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="HMS Discovery Port"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="HMS Discovery Port" dir=in action=allow protocol=udp localport=42010 remoteip=192.168.0.0/16,10.0.0.0/8,172.16.0.0/12 enable=yes'

!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Stop (but do not delete) the PostgreSQL service on uninstall, so patient
  ; data in C:\ProgramData\HMS\pgdata survives a reinstall/upgrade.
  SetShellVarContext all
  DetailPrint "Stopping PostgreSQL service..."
  nsExec::ExecToLog 'sc stop HMS-PostgreSQL'
!macroend
