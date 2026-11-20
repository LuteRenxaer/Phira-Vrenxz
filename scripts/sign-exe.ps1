# 给 Windows 构建产物做本地代码签名，消除「未知发布者 / 未包含有效数字签名」提示。
# 首次运行会生成自签名代码签名证书并装入当前用户的可信根与受信任发布者存储（可回滚：Cert:\CurrentUser 里删除该证书）。
param(
    [string[]]$Exe = @("F:\RustProject\phirLie\target\release\Phira-Vrenxz-main.exe"),
    [string]$Subject = "CN=Lute Rencai, O=Phira-Vrenxz, C=CN"
)

$ErrorActionPreference = "Stop"

$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
    Where-Object { $_.Subject -like "*$Subject*" -and $_.NotAfter -gt (Get-Date) } |
    Sort-Object NotAfter -Descending |
    Select-Object -First 1

if (-not $cert) {
    $cert = New-SelfSignedCertificate `
        -Type CodeSigningCert `
        -Subject $Subject `
        -KeyUsage DigitalSignature `
        -KeyLength 2048 `
        -HashAlgorithm SHA256 `
        -CertStoreLocation Cert:\CurrentUser\My `
        -NotAfter (Get-Date).AddYears(10)
    Write-Host "created cert: $($cert.Thumbprint)"
}

foreach ($store in @("Cert:\CurrentUser\Root", "Cert:\CurrentUser\TrustedPublisher")) {
    $existing = Get-ChildItem $store | Where-Object { $_.Thumbprint -eq $cert.Thumbprint }
    if (-not $existing) {
        $tmp = Join-Path $env:TEMP "pvn-sign-$($cert.Thumbprint).cer"
        Export-Certificate -Cert $cert -FilePath $tmp -Type CERT | Out-Null
        Import-Certificate -FilePath $tmp -CertStoreLocation $store | Out-Null
        Remove-Item $tmp -Force
        Write-Host "trusted in $store"
    }
}

foreach ($path in $Exe) {
    if (-not (Test-Path -LiteralPath $path)) {
        Write-Host "skip missing: $path"
        continue
    }
    $sig = Set-AuthenticodeSignature -LiteralPath $path -Certificate $cert -HashAlgorithm SHA256
    Write-Host ("{0} -> {1}" -f (Split-Path $path -Leaf), $sig.Status)
}
