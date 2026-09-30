use std::io;
use std::path::Path;

use crate::manifest::Manifest;

pub fn generate_restore_sh(manifest: &Manifest) -> String {
    let parts_list = manifest
        .parts
        .iter()
        .map(|p| format!("\"{}\"", p.filename))
        .collect::<Vec<_>>()
        .join(" ");

    let parts_cat = manifest
        .parts
        .iter()
        .map(|p| format!("\"$DIR/{}\"", p.filename))
        .collect::<Vec<_>>()
        .join(" ");

    let unpack_cmd = if manifest.mode == "fast" {
        format!(
            r#"if command -v zstd >/dev/null 2>&1; then
  cat {parts_cat} | zstd -d | tar -xf - -C "$DEST"
  echo "Restore complete!"
else
  echo "Error: 'zstd' command not found. Fast mode requires 'zstd' or the 'fastchunk' binary." >&2
  echo "Please install zstd or place 'fastchunk' in '$DIR'." >&2
  exit 1
fi"#
        )
    } else {
        format!(
            r#"cat {parts_cat} | tar -xf - -C "$DEST"
echo "Restore complete!""#
        )
    };

    format!(
        r#"#!/usr/bin/env sh
set -e

DIR="$(cd "$(dirname "$0")" && pwd)"
DEST="${{1:-.}}"

# Pre-flight check: verify all parts exist
MISSING=0
for PART in {parts_list}; do
  if [ ! -f "$DIR/$PART" ]; then
    echo "Error: Missing part '$PART' in '$DIR'" >&2
    MISSING=1
  fi
done

if [ "$MISSING" -ne 0 ]; then
  echo "Please copy missing part(s) into '$DIR' before restoring." >&2
  exit 1
fi

# Use fastchunk binary if present in directory
if [ -x "$DIR/fastchunk" ]; then
  echo "Using fastchunk binary..."
  "$DIR/fastchunk" restore "$DIR" -o "$DEST"
  exit $?
fi

mkdir -p "$DEST"
echo "Restoring to '$DEST' using system tools..."
{unpack_cmd}
"#
    )
}

pub fn generate_restore_bat(_manifest: &Manifest) -> String {
    r#"@echo off
setlocal
set "DIR=%~dp0"
if not "%~1"=="" (set "DEST=%~1") else (set "DEST=.")

if exist "%DIR%fastchunk.exe" (
  echo Using fastchunk binary...
  "%DIR%fastchunk.exe" restore "%DIR%" -o "%DEST%"
  exit /b %ERRORLEVEL%
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%DIR%restore.ps1" "%DEST%"
exit /b %ERRORLEVEL%
"#
    .to_string()
}

pub fn generate_restore_ps1(manifest: &Manifest) -> String {
    let parts_list = manifest
        .parts
        .iter()
        .map(|p| format!("  \"{}\"", p.filename))
        .collect::<Vec<_>>()
        .join(",\n");

    let unpack_cmd = if manifest.mode == "fast" {
        r#"if (Get-Command zstd -ErrorAction SilentlyContinue) {
  $partPaths = $Parts | ForEach-Object { Join-Path $Dir $_ }
  Get-Content -Path $partPaths -ReadCount 0 -Encoding Byte | & zstd -d | & tar.exe -xf - -C $Dest
  Write-Host "Restore complete!"
} else {
  Write-Error "Error: 'zstd' command not found. Fast mode requires 'zstd' or 'fastchunk.exe'."
  exit 1
}"#
    } else {
        r#"$partPaths = $Parts | ForEach-Object { Join-Path $Dir $_ }
Get-Content -Path $partPaths -ReadCount 0 -Encoding Byte | & tar.exe -xf - -C $Dest
Write-Host "Restore complete!""#
    };

    format!(
        r#"param(
  [string]$Dest = "."
)

$Dir = $PSScriptRoot
$Missing = @()
$Parts = @(
{parts_list}
)

foreach ($part in $Parts) {{
  $partPath = Join-Path $Dir $part
  if (-not (Test-Path $partPath)) {{
    $Missing += $part
  }}
}}

if ($Missing.Count -gt 0) {{
  Write-Error "Error: Missing part(s):`n$($Missing -join "`n")"
  Write-Host "Please copy missing part(s) into '$Dir' before restoring."
  exit 1
}}

if (Test-Path (Join-Path $Dir "fastchunk.exe")) {{
  Write-Host "Using fastchunk binary..."
  & (Join-Path $Dir "fastchunk.exe") restore $Dir -o $Dest
  exit $LASTEXITCODE
}}

if (-not (Test-Path $Dest)) {{
  New-Item -ItemType Directory -Path $Dest -Force | Out-Null
}}

Write-Host "Restoring to '$Dest' using Windows tar..."
{unpack_cmd}
"#
    )
}

pub fn write_restore_scripts(output_dir: &Path, manifest: &Manifest) -> io::Result<()> {
    std::fs::create_dir_all(output_dir)?;

    let sh_content = generate_restore_sh(manifest);
    let sh_path = output_dir.join("restore.sh");
    std::fs::write(&sh_path, sh_content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&sh_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&sh_path, perms)?;
    }

    let bat_content = generate_restore_bat(manifest);
    let bat_path = output_dir.join("restore.bat");
    std::fs::write(bat_path, bat_content)?;

    let ps1_content = generate_restore_ps1(manifest);
    let ps1_path = output_dir.join("restore.ps1");
    std::fs::write(ps1_path, ps1_content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunker::PartInfo;
    use tempfile::tempdir;

    #[test]
    fn test_script_generation_content() {
        let manifest = Manifest::new(
            "test_archive".to_string(),
            "directory".to_string(),
            2000,
            2,
            "fast".to_string(),
            1000,
            vec![
                PartInfo {
                    filename: "data.part001".to_string(),
                    size: 1000,
                    sha256: "hash1".to_string(),
                },
                PartInfo {
                    filename: "data.part002".to_string(),
                    size: 1000,
                    sha256: "hash2".to_string(),
                },
            ],
        );

        let sh = generate_restore_sh(&manifest);
        assert!(sh.contains("data.part001"));
        assert!(sh.contains("data.part002"));
        assert!(sh.contains("zstd"));
        assert!(sh.contains("fastchunk"));

        let bat = generate_restore_bat(&manifest);
        assert!(bat.contains("fastchunk.exe"));
        assert!(bat.contains("restore.ps1"));

        let ps1 = generate_restore_ps1(&manifest);
        assert!(ps1.contains("data.part001"));
        assert!(ps1.contains("data.part002"));
        assert!(ps1.contains("tar.exe"));
    }

    #[test]
    fn test_write_restore_scripts_creates_files() {
        let temp = tempdir().unwrap();
        let manifest = Manifest::new(
            "test_archive".to_string(),
            "directory".to_string(),
            100,
            1,
            "store".to_string(),
            100,
            vec![PartInfo {
                filename: "data.part001".to_string(),
                size: 100,
                sha256: "hash1".to_string(),
            }],
        );

        write_restore_scripts(temp.path(), &manifest).unwrap();

        let sh_path = temp.path().join("restore.sh");
        let bat_path = temp.path().join("restore.bat");
        let ps1_path = temp.path().join("restore.ps1");

        assert!(sh_path.exists());
        assert!(bat_path.exists());
        assert!(ps1_path.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&sh_path).unwrap();
            let mode = meta.permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "restore.sh should be executable");
        }
    }
}
