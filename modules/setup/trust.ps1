# LosOS: trust the certificate of the box "@HOST@" on this computer.
#
# What this script does, and all it does:
#
#   1. decodes the certificate at the bottom of this file;
#   2. checks that its SHA-256 fingerprint is
#        @FINGERPRINT@
#      which is the fingerprint the setup page shows you;
#   3. adds it to the Trusted Root store of your Windows account (not the
#      machine's), which Edge, Chrome and Firefox all read. Windows shows
#      its own confirmation dialog first; that dialog prints the same
#      certificate's SHA-1 thumbprint.
#
# It installs no software, changes no other setting, sends nothing anywhere
# and does not need to run as administrator.
#
# The box served this script from its own address on your network; nothing
# in it came from the internet. To undo it later, open "Manage user
# certificates" (certmgr.msc), Trusted Root Certification Authorities, and
# delete the entry issued to "@HOST@.local".

$ErrorActionPreference = 'Stop'

$BoxName = '@HOST@'
$Fingerprint = '@FINGERPRINT@'

$Pem = @'
@PEM@
'@

Write-Host "Trusting the certificate of $BoxName on this computer."
Write-Host "Fingerprint (SHA-256): $Fingerprint"
Write-Host "Compare it with the one the setup page shows. If they differ, close this window."

# Decode the PEM by hand rather than handing the text to the certificate
# constructor: both PowerShell editions then take the same path.
$base64 = ($Pem -split "`n" | Where-Object { $_ -notmatch '^-----' } | ForEach-Object { $_.Trim() }) -join ''
try {
  $der = [Convert]::FromBase64String($base64)
  $cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2 -ArgumentList (,$der)
} catch {
  Write-Error "The certificate in this script could not be read ($($_.Exception.Message)). Nothing was changed. Reload the setup page and try again."
  exit 1
}

$sha256 = [System.Security.Cryptography.SHA256]::Create()
$digest = ($sha256.ComputeHash($cert.RawData) | ForEach-Object { $_.ToString('X2') }) -join ':'
if ($digest -ne $Fingerprint) {
  Write-Error "The certificate in this script does not match its own fingerprint ($digest). Nothing was changed. Reload the setup page and try again."
  exit 1
}

$store = New-Object System.Security.Cryptography.X509Certificates.X509Store('Root', 'CurrentUser')
$store.Open('ReadWrite')
try {
  $store.Add($cert)
} finally {
  $store.Close()
}

Write-Host "Done. Restart any browser that is open, then reopen the setup page at https://$BoxName.local"
Write-Host "(or at the address you are using now, over https)."
